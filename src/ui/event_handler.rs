//! Crossterm event loop: input -> `Action` -> `App::update` -> render.

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

use crate::ui::app::{Action, App};
use crate::ui::widgets::agent_list;

/// Run the TUI until the user quits. Restores the terminal on all exits.
pub async fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3)])
                .split(f.area());
            agent_list::render(f, chunks[0], &app.agents, app.cursor);
        })?;

        if poll_action(&mut app)? {
            if app.should_quit {
                break Ok(());
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// Poll one key, apply to app. Returns true if a state change happened.
fn poll_action(app: &mut App) -> io::Result<bool> {
    if event::poll(Duration::from_millis(16))? {
        if let Event::Key(key) = event::read()? {
            let action = match key.code {
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
                KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
                KeyCode::Char(' ') => Action::ToggleSelectCurrent,
                // PRD: single 'a' toggles select/deselect all.
                KeyCode::Char('a') => {
                    if app.selected_count() == app.agents.len() {
                        Action::DeselectAll
                    } else {
                        Action::SelectAll
                    }
                }
                _ => Action::Noop,
            };
            app.update(action);
            return Ok(true);
        }
    }
    Ok(false)
}
