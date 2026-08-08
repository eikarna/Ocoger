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

use crate::ui::app::{Action, App, Mode};
use crate::ui::widgets::agent_list;

/// Run the TUI until the user quits. Restores the terminal on all exits.
pub async fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut app, &mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // header
                    Constraint::Min(3),    // agent list
                    Constraint::Length(6), // footer (log + hints)
                ])
                .split(f.area());

            let header = format!(
                " ocoger :: {} agent(s) | {} selected | {} unsaved ",
                app.agents.len(),
                app.selected_count(),
                app.dirty_count()
            );
            f.render_widget(ratatui::widgets::Paragraph::new(header), chunks[0]);

            agent_list::render(f, chunks[1], &app.agents, app.cursor);
            agent_list::render_bottom(f, chunks[2], &app.log);

            if app.mode == Mode::ModelEdit {
                agent_list::render_modal(
                    f,
                    f.area(),
                    &app.modal_input,
                    app.selected_count(),
                );
            }
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                handle_key(app, key);
                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    let action = match app.mode {
        Mode::ModelEdit => match key.code {
            KeyCode::Esc => Action::CancelModal,
            KeyCode::Enter => Action::ApplyModelModal,
            KeyCode::Backspace => Action::ModalBackspace,
            KeyCode::Char(c) => Action::ModalInput(c),
            _ => Action::Noop,
        },
        Mode::List => match (key.code, key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)) {
            (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => Action::Quit,
            // PRD: s or Ctrl+S both trigger save.
            (KeyCode::Char('s'), _) => Action::Save,
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => Action::MoveDown,
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => Action::MoveUp,
            (KeyCode::Char(' '), _) => Action::ToggleSelectCurrent,
            (KeyCode::Char('a'), _) => app.toggle_all_action(),
            (KeyCode::Char('m'), _) => Action::OpenModelModal,
            _ => Action::Noop,
        },
    };
    app.update(action);
}
