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
use crate::ui::widgets::{agent_list, diff_view, form, picker, preset_picker};

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
                Mode::Diff => {
                    diff_view::render(f, f.area(), app.diff_text.as_deref());
                }
                Mode::Preset | Mode::PresetNameNew | Mode::PresetConfirmAll => {
                    agent_list::render(f, chunks[1], &app.agents, app.cursor);
                    preset_picker::render(f, f.area(), app);
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
    use crate::core::keymap::{KeyCodeShape, KeySpec};
    // Translate crossterm KeyEvent into our pure KeySpec. Crossterm reports
    // SHIFT+A as `Char('A')` with SHIFT modifier; we canonicalize by
    // lowercasing when needed so "<S-a>" and "A" end up the same shape.
    let spec = match key.code {
        KeyCode::Char(c) => {
            let (c_norm, extra_shift) = (c, false);
            KeySpec {
                code: KeyCodeShape::Char(c_norm),
                ctrl: key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL),
                shift: key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT)
                    || extra_shift,
                alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
            }
        }
        KeyCode::Enter => KeySpec {
            code: KeyCodeShape::Enter,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Esc => KeySpec {
            code: KeyCodeShape::Esc,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Backspace => KeySpec {
            code: KeyCodeShape::Backspace,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Tab => KeySpec {
            code: KeyCodeShape::Tab,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Up => KeySpec {
            code: KeyCodeShape::Up,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Down => KeySpec {
            code: KeyCodeShape::Down,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Left => KeySpec {
            code: KeyCodeShape::Left,
            ctrl: false,
            shift: false,
            alt: false,
        },
        KeyCode::Right => KeySpec {
            code: KeyCodeShape::Right,
            ctrl: false,
            shift: false,
            alt: false,
        },
        _ => return Action::Noop,
    };

    // First try an exact lookup. If none, fall back to an alternative that
    // treats an uppercase char as shift-modified lowercase (crossterm on
    // Windows reports `P` + SHIFT for both `<S-p>` and bare `P`).
    let action = app.keymap.lookup(app.mode, spec).or_else(|| {
        if let (KeyCodeShape::Char(c), true) = (spec.code, spec.shift) {
            // Try shift=true with the lowercase form, and shift=false as typed.
            let lower = spec_with(
                KeyCodeShape::Char(c.to_ascii_lowercase()),
                spec.ctrl,
                true,
                spec.alt,
            );
            let upper_no_shift = spec_with(KeyCodeShape::Char(c), spec.ctrl, false, spec.alt);
            app.keymap
                .lookup(app.mode, lower)
                .or_else(|| app.keymap.lookup(app.mode, upper_no_shift))
        } else if let KeyCodeShape::Char(c) = spec.code {
            // Typing 'a' may also match a user keymap entry `<S-a>` if crossterm
            // didn't pass SHIFT through.
            let shifted = spec_with(KeyCodeShape::Char(c), spec.ctrl, true, spec.alt);
            app.keymap.lookup(app.mode, shifted)
        } else {
            None
        }
    });

    let action = match action {
        Some(a) => a,
        None => {
            // Modal fallthroughs: these modes accept free-text input, so any
            // Char that isn't bound to an action still routes to the buffer.
            match (app.mode, key.code) {
                (Mode::ModelEdit, KeyCode::Char(c)) => Action::ModalInput(c),
                (Mode::Picker, KeyCode::Char(c)) => Action::PickerInput(c),
                (Mode::Preset, KeyCode::Char(c)) => Action::PresetInput(c),
                (Mode::PresetNameNew, KeyCode::Char(c)) => Action::PresetInput(c),
                (Mode::ModelEdit, KeyCode::Backspace) => Action::ModalBackspace,
                (Mode::Picker, KeyCode::Backspace) => Action::PickerBackspace,
                (Mode::Preset, KeyCode::Backspace) => Action::PresetBackspace,
                (Mode::PresetNameNew, KeyCode::Backspace) => Action::PresetBackspace,
                _ => Action::Noop,
            }
        }
    };

    app.update(action.clone());
    action
}

fn spec_with(
    code: crate::core::keymap::KeyCodeShape,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> crate::core::keymap::KeySpec {
    crate::core::keymap::KeySpec {
        code,
        ctrl,
        shift,
        alt,
    }
}
