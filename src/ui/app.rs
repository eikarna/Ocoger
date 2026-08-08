//! MVU model & update for the Ocoger TUI. Pure (no crossterm) so it is
//! unit-testable; the event loop in `event_handler.rs` just maps keys to
//! `Action`s and re-renders.

use crate::core::agent_parser::AgentFile;
use crate::core::jsonc_config::{ConfigItem, JsoncConfig};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    List,
    /// Batch model input modal (PRD FE-1.3). Value staged in `modal_input`.
    ModelEdit,
    /// Form editing: agent params + global config (single shared cursor).
    Form,
    /// Model picker with live-filter (Phase 2 FE-3.3).
    Picker,
}

/// Event-loop async actions the pure `update()` cannot perform itself (spawn/
/// kill are async I/O); returned instead of being executed so `update()`
/// remains testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartSignal {
    None,
    Requested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    AgentParams,
    GlobalConfig,
}

#[derive(Debug, Clone)]
pub enum Action {
    MoveDown,
    MoveUp,
    ToggleSelectCurrent,
    SelectAll,
    DeselectAll,
    OpenModelModal,
    OpenForm,
    OpenPicker,
    FormMove(bool),
    FormModify(i32),
    FormApply,
    FormExit,
    PickerInput(char),
    PickerBackspace,
    PickerAccept,
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
    /// Force-restart process (PRD `r`).
    Restart,
    Quit,
    Noop,
}

pub struct App {
    pub agents: Vec<AgentFile>,
    pub cursor: usize,
    pub should_quit: bool,
    pub project_root: PathBuf,
    pub mode: Mode,
    /// Staged string while in `ModelEdit` or `Picker` mode.
    pub modal_input: String,
    /// Form band (agent params vs global config) whose cursor is active.
    pub form_band: Panel,
    /// Current row index within the active band.
    pub form_cursor: usize,
    /// Global config rows + CST editor handle (None when no config file on disk).
    pub config_items: Vec<ConfigItem>,
    pub config: Option<JsoncConfig>,
    /// Catalog for the picker (static fallback + fetched).
    pub picker_catalog: Vec<String>,
    /// Filtered view of picker_catalog for rendering.
    pub picker_items: Vec<String>,
    /// Picker row after live filter.
    pub picker_cursor: usize,
    /// Rolling status/messages for the footer log line.
    pub log: Vec<String>,
    /// Shared live catalog fed by the background model fetcher task.
    shared_catalog: std::sync::Arc<tokio::sync::RwLock<std::collections::HashSet<String>>>,
    /// Guards against re-logging the same fetch merge repeatedly.
    fetch_logged: bool,
}

impl App {
    pub fn new(agents: Vec<AgentFile>, project_root: PathBuf) -> Self {
        let mut config = None;
        if let Ok(Some(c)) = JsoncConfig::load(&project_root) {
            config = Some(c);
        }
        let config_items = config
            .as_ref()
            .and_then(|c| c.config_items().ok())
            .unwrap_or_default();
        // Boot with the static Anthropic catalog; live fetches merge in later
        // when the async refresh completes (services::model_fetcher).
        let picker_items: Vec<String> = crate::services::model_fetcher::ANTHROPIC_NATIVE_MODELS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut app = Self {
            agents,
            cursor: 0,
            should_quit: false,
            project_root,
            mode: Mode::List,
            modal_input: String::new(),
            form_band: Panel::AgentParams,
            form_cursor: 0,
            config_items,
            config,
            picker_catalog: picker_items.clone(),
            picker_items,
            picker_cursor: 0,
            log: Vec::new(),
            shared_catalog: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            fetch_logged: false,
        };
        app.spawn_catalog_fetch();
        app
    }

    /// Provider base URLs extracted from the loaded config for model fetch.
    fn provider_base_urls(&self) -> Vec<String> {
        self.config_items
            .iter()
            .filter(|i| i.label.ends_with(".base_url"))
            .map(|i| i.value.clone())
            .filter(|v| !v.is_empty())
            .collect()
    }

    /// Spawn a background refresh; results feed `shared_catalog`.
    pub fn spawn_catalog_fetch(&self) {
        use crate::services::model_fetcher;
        let urls = self.provider_base_urls();
        if urls.is_empty() {
            return;
        }
        let shared = self.shared_catalog.clone();
        // Read API key pointer if exposed via config (env var name). Currently
        // heuristic (TODO Phase 2 polish): look for any *_api_key env name.
        let api_key_env = self
            .config_items
            .iter()
            .find(|i| i.label.ends_with(".api_key"))
            .map(|i| i.value.clone())
            .filter(|v| !v.is_empty());
        tokio::spawn(async move {
            let results = model_fetcher::refresh_catalog(urls, shared, api_key_env).await;
            tracing::info!(?results, "model catalog refresh finished");
        });
    }

    /// Merge shared live results into the picker-visible catalog (deduped,
    /// sorted). Only logs once per change batch.
    pub fn sync_catalog_from_shared(&mut self) {
        let snap = match self.shared_catalog.try_read() {
            Ok(g) => g.clone(),
            Err(_) => return,
        };
        if snap.is_empty() {
            return;
        }
        let before = self.picker_catalog.len();
        let mut set: std::collections::HashSet<String> =
            self.picker_catalog.iter().cloned().collect();
        set.extend(snap);
        let mut merged: Vec<String> = set.into_iter().collect();
        merged.sort();
        if merged.len() != before || merged != self.picker_catalog {
            self.picker_catalog = merged;
            self.reload_picker_view();
            if !self.fetch_logged {
                self.log(format!(
                    "{} live model(s) merged into catalog",
                    self.picker_catalog.len()
                ));
                self.fetch_logged = true;
            }
        }
    }

    /// Reload filtered picker view after catalog or filter change.
    fn reload_picker_view(&mut self) {
        let filter = self.modal_input.to_lowercase();
        self.picker_items = self
            .picker_catalog
            .iter()
            .filter(|m| filter.is_empty() || m.to_lowercase().contains(&filter))
            .cloned()
            .collect();
        if !self.picker_items.is_empty() && self.picker_cursor >= self.picker_items.len() {
            self.picker_cursor = self.picker_items.len() - 1;
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
        let _typed = !self.modal_input.trim().is_empty();

        use Action::*;
        match action {
            MoveDown | MoveUp => match self.mode {
                Mode::List => self.move_cursor(matches!(action, MoveDown)),
                Mode::Form => {
                    let n = self.form_item_count();
                    if n > 0 {
                        self.form_cursor = if matches!(action, MoveDown) {
                            (self.form_cursor + 1) % n
                        } else {
                            (self.form_cursor + n - 1) % n
                        };
                    }
                }
                Mode::ModelEdit | Mode::Picker => {}
            },
            OpenForm => {
                if self.mode == Mode::List {
                    self.form_band = Panel::AgentParams;
                    self.form_cursor = 0;
                    self.mode = Mode::Form;
                }
            }
            OpenPicker => match self.mode {
                Mode::List if self.selected_count() > 0 && !self.picker_catalog.is_empty() => {
                    self.modal_input.clear();
                    self.picker_cursor = 0;
                    self.mode = Mode::Picker;
                }
                Mode::Form => {
                    if self.picker_catalog.is_empty() {
                        self.log(
                            "Model catalog is empty; fetch models first (Phase 2 fetcher)"
                                .to_string(),
                        );
                    }
                    // Entering picker from Form only makes sense on the model band. Keep
                    // mode unchanged; UI can still show filtered catalog if desired.
                }
                _ => {}
            },
            FormMove(next) => {
                if self.mode != Mode::Form {
                    return;
                }
                let n = self.form_item_count();
                if n > 0 {
                    self.form_cursor = if next {
                        (self.form_cursor + 1) % n
                    } else {
                        (self.form_cursor + n - 1) % n
                    };
                }
            }
            FormModify(d) => self.modify_form_cursor(d),
            FormApply => {} // reserved for two-step (currently edits apply immediately)
            FormExit => {
                if self.mode == Mode::Form {
                    self.mode = Mode::List;
                }
            }
            PickerInput(c) => {
                if self.mode == Mode::Picker {
                    self.modal_input.push(c);
                    self.reload_picker_view();
                }
            }
            PickerBackspace => {
                if self.mode == Mode::Picker {
                    self.modal_input.pop();
                    self.reload_picker_view();
                }
            }
            PickerAccept => {
                if self.mode == Mode::Picker {
                    self.picker_apply_selection();
                    self.mode = Mode::List;
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
            OpenModelModal => {
                if self.mode == Mode::List && self.selected_count() > 0 {
                    self.modal_input.clear();
                    self.mode = Mode::ModelEdit;
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
            Save => {
                if self.mode == Mode::List {
                    self.save_dirty();
                }
            }
            Restart => {
                // Process restart is handled by the event loop (async I/O);
                // App only updates the log so the model stays testable/pure.
                if self.mode == Mode::List {
                    self.log("restart requested (event loop will handle)".to_string());
                }
            }
            CancelModal => {
                // Discard staged input for ModelEdit or Picker.
                if matches!(self.mode, Mode::ModelEdit | Mode::Picker) {
                    self.modal_input.clear();
                    self.mode = Mode::List;
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

    pub fn form_item_count(&self) -> usize {
        match self.form_band {
            Panel::AgentParams => 5, // model, temperature, top_k, top_p, reasoning_effort
            Panel::GlobalConfig => self.config_items.len(),
        }
    }

    fn agent_param_label(idx: usize) -> &'static str {
        const L: [&str; 5] = ["model", "temperature", "top_k", "top_p", "reasoning_effort"];
        L.get(idx).copied().unwrap_or("?")
    }

    /// +/- step on the current form row. Applies to selected agents or the
    /// global config item immediately (config mutation is CST-preserving).
    fn modify_form_cursor(&mut self, d: i32) {
        if self.mode != Mode::Form || self.form_cursor >= self.form_item_count() {
            return;
        }
        if self.form_band == Panel::GlobalConfig {
            if let Some(err) = self.config_set_current_value(d) {
                self.log(err);
            }
            return;
        }
        // Agent parameters.
        let key = Self::agent_param_label(self.form_cursor);
        let step_map: &[(&str, &str)] = match key {
            "temperature" => &[("0.1", "0.1")],
            "top_p" => &[("0.05", "0.05")],
            _ => &[],
        };
        let Some(step_str) = step_map.iter().find(|(k, _)| *k == key).map(|(_, s)| s) else {
            // keys not editable with +/- (model/top_k/reasoning_effort handled via picker/typing)
            return;
        };
        let step: f32 = step_str.parse().unwrap_or(0.0);
        let cur = self
            .agents
            .get(self.cursor)
            .and_then(|a| match key {
                "temperature" => a.frontmatter.temperature,
                "top_p" => a.frontmatter.top_p,
                _ => None,
            })
            .unwrap_or_else(|| if key == "top_p" { 0.9 } else { 0.2 });
        let new_val = ((cur + d as f32 * step) * 100.0).round() / 100.0;
        let clamped = if key == "temperature" {
            new_val.clamp(0.0, 2.0)
        } else {
            new_val.clamp(0.0, 1.0)
        };
        let formatted = format!("{clamped:.2}");
        let fields = [(key.to_string(), formatted.clone())];
        let target = self.agents.iter_mut().filter(|a| a.is_selected);
        for a in target {
            a.update_models(&fields);
        }
        self.log(format!(
            "Set {key}={formatted} for selected agents (press s to save)"
        ));
    }

    fn config_set_current_value(&mut self, d: i32) -> Option<String> {
        let item = self.config_items.get(self.form_cursor)?.clone();
        if let Some(cfg) = self.config.as_mut() {
            // single-char preview → better to have a small +/- helper for now.
            let cur: i32 = item.value.parse().unwrap_or(0);
            let new_val = (cur + d).to_string();
            if let Err(e) = cfg.set_nested_str(item.keypath.iter().map(|s| s.as_str()), &new_val) {
                return Some(format!("config set {} failed: {e}", item.label));
            }
        }
        self.config_items = self.config.as_ref()?.config_items().ok()?;
        None
    }

    fn picker_apply_selection(&mut self) {
        let Some(model) = self.picker_items.get(self.picker_cursor).cloned() else {
            self.log("Picker catalogue empty".to_string());
            return;
        };
        let fields = vec![("model".to_string(), model.clone())];
        for a in self.agents.iter_mut().filter(|a| a.is_selected) {
            a.update_models(&fields);
        }
        self.log(format!(
            "Staged model '{model}' on {} agent(s) via picker (press s to save)",
            self.selected_count()
        ));
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

    /// Public log-push used by the event loop for process/log wiring.
    pub fn log_push(&mut self, msg: impl Into<String>) {
        self.log(msg);
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

    /// Save all dirty agents, then signal whether the supervised process
    /// should be restarted (event loop observes this on `s` / `Ctrl+S`).
    /// Returns `true` only when at least one file was written.
    pub fn save_and_check_restart(&mut self) -> bool {
        let changed = self.dirty_count() > 0;
        self.save_dirty();
        changed && self.dirty_count() == 0
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

    #[test]
    fn picker_boots_with_anthropic_fallback_and_filters() {
        let app = App::new(vec![agent("a")], PathBuf::from("."));
        assert!(!app.picker_catalog.is_empty(), "boot populates fallback");
        assert!(app.picker_catalog.iter().any(|m| m.contains("claude")));
        assert_eq!(app.picker_items.len(), app.picker_catalog.len());
    }

    #[test]
    fn picker_filter_narrows_then_accept_applies_to_selected() {
        let mut app = App::new(vec![agent("a")], PathBuf::from("."));
        app.agents[0].is_selected = true;
        app.update(Action::OpenPicker);
        assert_eq!(app.mode, Mode::Picker);
        // filter to "opus"
        for c in "opus".chars() {
            app.update(Action::PickerInput(c));
        }
        assert!(
            app.picker_items.iter().all(|m| m.contains("opus")),
            "filter applied"
        );
        assert_eq!(
            app.picker_items.len(),
            1,
            "exactly one opus model in static list"
        );
        app.update(Action::PickerAccept);
        assert_eq!(app.mode, Mode::List);
        assert!(app.agents[0].frontmatter.model.contains("opus"));
        assert!(app.agents[0].is_dirty, "pick stages a dirty edit");
    }

    #[test]
    fn sync_catalog_merges_shared_results_dedup_sorted() {
        let mut app = App::new(vec![agent("a")], PathBuf::from("."));
        let before = app.picker_catalog.len();
        {
            let mut w = app.shared_catalog.blocking_write();
            w.insert("deepseek-r1".to_string());
            w.insert("claude-3-5-sonnet-20241022".to_string()); // already present
            w.insert("ollama/llama3.2".to_string());
        }
        app.sync_catalog_from_shared();
        assert_eq!(app.picker_catalog.len(), before + 2);
        assert!(app.picker_catalog.contains(&"deepseek-r1".to_string()));
        assert!(app.picker_catalog.contains(&"ollama/llama3.2".to_string()));
        assert_eq!(
            app.picker_catalog
                .iter()
                .filter(|m| *m == "claude-3-5-sonnet-20241022")
                .count(),
            1,
            "dedup: shared result already in static list merges once"
        );
        let mut sorted = app.picker_catalog.clone();
        sorted.sort();
        assert_eq!(app.picker_catalog, sorted, "catalog is sorted");
        assert!(app.log.iter().any(|m| m.contains("live model(s) merged")));
    }
}
