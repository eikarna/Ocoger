//! MVU model & update for the Ocoger TUI. Pure (no crossterm) so it is
//! unit-testable; the event loop in `event_handler.rs` just maps keys to
//! `Action`s and re-renders.

use crate::core::agent_parser::AgentFile;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    ToggleSelectCurrent,
    SelectAll,
    DeselectAll,
    Quit,
    Noop,
}

pub struct App {
    pub agents: Vec<AgentFile>,
    pub cursor: usize,
    pub should_quit: bool,
    pub project_root: PathBuf,
}

impl App {
    pub fn new(agents: Vec<AgentFile>, project_root: PathBuf) -> Self {
        Self {
            agents,
            cursor: 0,
            should_quit: false,
            project_root,
        }
    }

    pub fn selected_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_selected).count()
    }

    pub fn update(&mut self, action: Action) {
        use Action::*;
        match action {
            MoveDown | MoveUp => self.move_cursor(action == MoveDown),
            ToggleSelectCurrent => self.toggle(),
            SelectAll => self.set_all(true),
            DeselectAll => self.set_all(false),
            Quit => self.should_quit = true,
            Noop => {}
        }
    }

    fn move_cursor(&mut self, down: bool) {
        let len = self.agents.len();
        if len == 0 {
            return;
        }
        self.cursor = if down {
            (self.cursor + 1) % len
        } else {
            (self.cursor + len - 1) % len
        };
    }

    fn toggle(&mut self) {
        if let Some(a) = self.agents.get_mut(self.cursor) {
            a.is_selected = !a.is_selected;
        }
    }

    fn set_all(&mut self, val: bool) {
        for a in &mut self.agents {
            a.is_selected = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_parser::{AgentFile, AgentFrontmatter};
    use std::path::PathBuf;

    fn agent(name: &str) -> AgentFile {
        AgentFile {
            path: PathBuf::from(name),
            frontmatter: AgentFrontmatter {
                model: "m".into(),
                temperature: None,
                top_k: None,
                top_p: None,
                reasoning_effort: None,
            },
            raw_body: String::new(),
            is_selected: false,
            raw_yaml: String::new(),
        }
    }

    #[test]
    fn navigation_and_selection() {
        let mut app = App::new(vec![agent("a"), agent("b"), agent("c")], PathBuf::from("."));
        assert_eq!(app.cursor, 0);
        app.update(Action::MoveDown);
        assert_eq!(app.cursor, 1);
        app.update(Action::MoveUp);
        assert_eq!(app.cursor, 0);
        app.update(Action::MoveUp);
        assert_eq!(app.cursor, 2, "wraps");
        app.update(Action::ToggleSelectCurrent);
        assert!(app.agents[2].is_selected);
        app.update(Action::SelectAll);
        assert_eq!(app.selected_count(), 3);
        app.update(Action::DeselectAll);
        assert_eq!(app.selected_count(), 0);
    }
}
