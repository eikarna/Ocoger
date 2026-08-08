//! MVU model & update for the Ocoger TUI. Pure (no crossterm) so it is
//! unit-testable; the event loop in `event_handler.rs` just maps keys to
//! `Action`s and re-renders.

use crate::core::agent_parser::AgentFile;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    List,
    /// Batch model input modal (PRD FE-1.3). Value staged in `modal_input`.
    ModelEdit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    ToggleSelectCurrent,
    SelectAll,
    DeselectAll,
    OpenModelModal,
    /// Append chars to the staged model string (modal only).
    ModalInput(char),
    /// Pop one char from the staged model string (modal only).
    ModalBackspace,
    /// Apply staged model to all selected agents, close modal.
    ApplyModelModal,
    /// Abort modal without applying.
    CancelModal,
    /// Save all dirty agents atomically (PRD §4: s / Ctrl+S).
    Save,
    Quit,
    Noop,
}

pub struct App {
    pub agents: Vec<AgentFile>,
    pub cursor: usize,
    pub should_quit: bool,
    pub project_root: PathBuf,
    pub mode: Mode,
    /// Staged model string while in `ModelEdit` mode.
    pub modal_input: String,
    /// Rolling status/messages for the footer log line.
    pub log: Vec<String>,
}

impl App {
    pub fn new(agents: Vec<AgentFile>, project_root: PathBuf) -> Self {
        Self {
            agents,
            cursor: 0,
            should_quit: false,
            project_root,
            mode: Mode::List,
            modal_input: String::new(),
            log: Vec::new(),
        }
    }

    pub fn selected_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_selected).count()
    }

    pub fn dirty_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_dirty).count()
    }

    /// `a` toggles select-all vs deselect-all (PRD §4 single-key toggle).
    pub fn toggle_all_action(&self) -> Action {
        if self.agents.is_empty() {
            return Action::Noop;
        }
        if self.selected_count() == self.agents.len() {
            Action::DeselectAll
        } else {
            Action::SelectAll
        }
    }

    pub fn update(&mut self, action: Action) {
        use Action::*;
        match action {
            MoveDown | MoveUp => {
                if self.mode == Mode::List {
                    self.move_cursor(action == MoveDown)
                }
            }
            ToggleSelectCurrent => {
                if self.mode == Mode::List {
                    self.toggle()
                }
            }
            SelectAll => {
                if self.mode == Mode::List {
                    self.set_all(true)
                }
            }
            DeselectAll => {
                if self.mode == Mode::List {
                    self.set_all(false)
                }
            }
            OpenModelModal => {
                if self.mode == Mode::List && self.selected_count() > 0 {
                    self.modal_input.clear();
                    self.mode = Mode::ModelEdit;
                }
            }
            ModalInput(c) => {
                if self.mode == Mode::ModelEdit {
                    self.modal_input.push(c);
                }
            }
            ModalBackspace => {
                if self.mode == Mode::ModelEdit {
                    self.modal_input.pop();
                }
            }
            ApplyModelModal => {
                if self.mode == Mode::ModelEdit {
                    self.apply_staged_model();
                    self.mode = Mode::List;
                }
            }
            CancelModal => {
                if self.mode == Mode::ModelEdit {
                    self.modal_input.clear();
                    self.mode = Mode::List;
                }
            }
            Save => {
                if self.mode == Mode::List {
                    self.save_dirty();
                }
            }
            Quit => {
                // Guard against data loss (BRD: don't destroy unsaved edits).
                if self.dirty_count() == 0 {
                    self.should_quit = true;
                } else {
                    self.log(format!(
                        "Unsaved changes ({} agent(s)); press s to save, or s then q",
                        self.dirty_count()
                    ));
                }
            }
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

    fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        // Keep the footer bounded.
        if self.log.len() > 5 {
            let drop_n = self.log.len() - 5;
            self.log.drain(..drop_n);
        }
    }

    fn apply_staged_model(&mut self) {
        let model = self.modal_input.trim().to_string();
        if model.is_empty() {
            self.log("Empty model ignored".to_string());
            return;
        }
        let fields = vec![("model".to_string(), model.clone())];
        let mut count = 0;
        for a in self.agents.iter_mut().filter(|a| a.is_selected) {
            a.update_models(&fields);
            count += 1;
        }
        self.log(format!(
            "Staged model '{model}' on {count} agent(s); press s to save"
        ));
    }

    fn save_dirty(&mut self) {
        let mut ok = 0;
        let mut failed = vec![];
        for a in self.agents.iter_mut().filter(|a| a.is_dirty) {
            match a.save() {
                Ok(()) => ok += 1,
                Err(e) => {
                    failed.push(format!(
                        "{}: {e}",
                        a.path
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| a.path.display().to_string())
                    ));
                }
            }
        }
        if ok > 0 {
            self.log(format!("Saved {ok} agent(s)"));
        }
        if !failed.is_empty() {
            self.log(format!("SAVE FAILED: {}", failed.join(", ")));
        }
        if ok == 0 && failed.is_empty() {
            self.log("Nothing to save".to_string());
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
            path: PathBuf::from(format!("{name}.md")),
            frontmatter: AgentFrontmatter {
                model: "m".into(),
                temperature: None,
                top_k: None,
                top_p: None,
                reasoning_effort: None,
            },
            raw_body: String::new(),
            is_selected: false,
            is_dirty: false,
            raw_yaml: "model: m".into(),
        }
    }

    fn app3() -> App {
        App::new(vec![agent("a"), agent("b"), agent("c")], PathBuf::from("."))
    }

    #[test]
    fn navigation_and_selection() {
        let mut app = app3();
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

    #[test]
    fn batch_apply_marks_selected_dirty() {
        let mut app = app3();
        // select a and c
        app.agents[0].is_selected = true;
        app.agents[2].is_selected = true;
        app.update(Action::OpenModelModal);
        assert_eq!(app.mode, Mode::ModelEdit);
        for c in "deepseek-r1".chars() {
            app.update(Action::ModalInput(c));
        }
        app.update(Action::ModalBackspace); // drop trailing '1'
        app.update(Action::ModalInput('2'));
        app.update(Action::ApplyModelModal);
        assert_eq!(app.mode, Mode::List);
        assert_eq!(app.agents[0].frontmatter.model, "deepseek-r2");
        assert_eq!(app.agents[2].frontmatter.model, "deepseek-r2");
        assert_eq!(app.agents[1].frontmatter.model, "m", "unselected untouched");
        assert_eq!(app.dirty_count(), 2);
        assert!(app
            .log
            .iter()
            .any(|m| m.contains("Staged model 'deepseek-r2' on 2")));
    }

    #[test]
    fn modal_cancel_discards_input() {
        let mut app = app3();
        app.agents[0].is_selected = true;
        app.update(Action::OpenModelModal);
        app.update(Action::ModalInput('x'));
        app.update(Action::CancelModal);
        assert_eq!(app.mode, Mode::List);
        assert_eq!(app.modal_input, "");
        assert_eq!(app.dirty_count(), 0);
    }

    #[test]
    fn quit_guard_blocks_with_dirty() {
        let mut app = app3();
        app.agents[0].is_dirty = true;
        app.update(Action::Quit);
        assert!(!app.should_quit);
        assert!(app.log.iter().any(|m| m.contains("Unsaved changes")));
        app.agents[0].is_dirty = false;
        app.update(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn save_writes_selected_files_and_clears_dirty() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-app-e2e-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // two agents on disk
        let mk = |name: &str, model: &str| {
            let p = dir.join(format!("{name}.md"));
            std::fs::write(&p, format!("---\nmodel: {model}\n---\nbody\n")).unwrap();
            crate::core::agent_parser::load_agent(&p).unwrap()
        };
        let mut app = App::new(vec![mk("a", "m1"), mk("b", "m2")], dir.clone());
        app.agents[0].is_selected = true;
        app.update(Action::OpenModelModal);
        for c in "openai/gpt-9".chars() {
            app.update(Action::ModalInput(c));
        }
        app.update(Action::ApplyModelModal);
        app.update(Action::Save);
        assert_eq!(app.dirty_count(), 0);
        let disk_a = std::fs::read_to_string(dir.join("a.md")).unwrap();
        assert!(disk_a.contains("model: openai/gpt-9"));
        let disk_b = std::fs::read_to_string(dir.join("b.md")).unwrap();
        assert!(disk_b.contains("model: m2"), "unselected agent untouched");
    }
}
