//! Crossterm event loop: input -> `Action` -> `App::update` -> render.

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

use crate::services::process_manager::ProcessManager;
use crate::ui::app::{Action, App, Mode};
use crate::ui::widgets::{agent_list, form, picker};

/// Run the TUI until the user quits. Restores the terminal on all exits.
pub async fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut proc_mgr = ProcessManager::new();
    let result = event_loop(&mut app, &mut terminal, &mut proc_mgr).await;
    proc_mgr.shutdown_sync();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn event_loop(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    proc_mgr: &mut ProcessManager,
) -> io::Result<()> {
    let mut last_proc_state = proc_mgr.state;
    loop {
        app.sync_catalog_from_shared();
        for (stream, line) in proc_mgr.drain_output() {
            app.log_push(format!("[{stream}] {line}"));
        }
        if proc_mgr.state != last_proc_state {
            app.log_push(format!(
                "process state: {:?} -> {:?}",
                last_proc_state, proc_mgr.state
            ));
            last_proc_state = proc_mgr.state;
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // header
                    Constraint::Min(3),    // content
                    Constraint::Length(6), // footer
                ])
                .split(f.area());

            let proc_lbl = match proc_mgr.state {
                crate::services::process_manager::ProcState::Running => {
                    format!("RUNNING pid={:?}", proc_mgr.pid)
                }
                s => format!("{:?}", s),
            };
            let header = format!(
                " ocoger :: {} agent(s) | {} selected | {} unsaved | mode: {:?} | proc: {} ",
                app.agents.len(),
                app.selected_count(),
                app.dirty_count(),
                app.mode,
                proc_lbl
            );
            f.render_widget(ratatui::widgets::Paragraph::new(header), chunks[0]);

            match app.mode {
                Mode::List | Mode::ModelEdit => {
                    agent_list::render(f, chunks[1], &app.agents, app.cursor);
                    if app.mode == Mode::ModelEdit {
                        agent_list::render_modal(
                            f,
                            f.area(),
                            &app.modal_input,
                            app.selected_count(),
                        );
                    }
                }
                Mode::Form => form::render(f, chunks[1], app),
                Mode::Picker => {
                    agent_list::render(f, chunks[1], &app.agents, app.cursor);
                    picker::render(f, f.area(), app);
                }
            }

            agent_list::render_bottom(f, chunks[2], &app.log);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                let action = handle_key(app, key);
                maybe_restart(proc_mgr, app, action).await;
                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}

/// If `action` is a process-restarting action (r / save with changes), perform
/// it and log the result.
async fn maybe_restart(proc_mgr: &mut ProcessManager, app: &mut App, action: Action) {
    use Action::*;
    let want_restart = match action {
        Restart => true,
        Save => app.save_and_check_restart(),
        _ => return,
    };
    if !want_restart {
        return;
    }
    let cwd = app.project_root.clone();
    app.log_push(format!("restarting process (pid={:?})…", proc_mgr.pid));
    match proc_mgr.restart(&cwd).await {
        Ok(new_pid) => app.log_push(format!("restarted → pid={new_pid}")),
        Err(e) => app.log_push(format!("restart failed: {e}")),
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    let action = match app.mode {
        Mode::ModelEdit => match key.code {
            KeyCode::Esc => Action::CancelModal,
            KeyCode::Enter => Action::ApplyModelModal,
            KeyCode::Backspace => Action::ModalBackspace,
            KeyCode::Char(c) => Action::ModalInput(c),
            _ => Action::Noop,
        },
        Mode::Picker => match key.code {
            KeyCode::Esc => Action::CancelModal,
            KeyCode::Enter => Action::PickerAccept,
            KeyCode::Backspace => Action::PickerBackspace,
            KeyCode::Char(c) => Action::PickerInput(c),
            _ => Action::Noop,
        },
        Mode::Form => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::FormExit,
            KeyCode::Char('j') | KeyCode::Down => Action::FormMove(true),
            KeyCode::Char('k') | KeyCode::Up => Action::FormMove(false),
            KeyCode::Tab => Action::FormExit, // TODO Tab pane-switch in later phase
            KeyCode::Char('e') | KeyCode::Char('g') => Action::FormExit,
            KeyCode::Char('+') | KeyCode::Char('=') => Action::FormModify(1),
            KeyCode::Char('-') => Action::FormModify(-1),
            KeyCode::Enter => Action::FormApply,
            _ => Action::Noop,
        },
        Mode::List => match (
            key.code,
            key.modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
        ) {
            (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => Action::Quit,
            (KeyCode::Char('s'), _) => Action::Save,
            (KeyCode::Char('r'), _) => Action::Restart,
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => Action::MoveDown,
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => Action::MoveUp,
            (KeyCode::Char(' '), _) => Action::ToggleSelectCurrent,
            (KeyCode::Char('a'), _) => app.toggle_all_action(),
            (KeyCode::Char('m'), _) => Action::OpenModelModal,
            (KeyCode::Char('p'), _) => Action::OpenPicker,
            (KeyCode::Char('e'), _) | (KeyCode::Char('g'), _) => Action::OpenForm,
            _ => Action::Noop,
        },
    };
    app.update(action.clone());
    action
}
