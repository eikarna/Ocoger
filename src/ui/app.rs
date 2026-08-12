//! MVU model & update for the Ocoger TUI. Pure (no crossterm) so it is
//! unit-testable; the event loop in `event_handler.rs` just maps keys to
//! `Action`s and re-renders.

use crate::core::agent_parser::AgentFile;
use crate::core::config_resolver;
use crate::core::jsonc_config::{ConfigItem, JsoncConfig};
use crate::core::keymap::Keymap;
use crate::core::presets::{Preset, Presets};
use crate::ui::theme::Theme;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Phase 5 hub: boot menu dispatching into pane modes. `List` stays as
    /// the Subagents pane (enum kept to avoid churn across call sites).
    MainMenu,
    List,
    /// Batch model input modal (PRD FE-1.3). Value staged in `modal_input`.
    ModelEdit,
    /// Form editing: agent params + global config (single shared cursor).
    Form,
    /// Model picker with live-filter (Phase 2 FE-3.3).
    Picker,
    /// Pre-save diff review (PRD §9). Read-only; Enter/Esc closes.
    Diff,
    /// Preset picker modal (Phase 4). Live-filtered list of presets; Enter
    /// applies to selected agents, `a` prompts apply-to-all, `n` captures from
    /// selection, `d` deletes the highlighted preset.
    Preset,
    /// Name-entry modal for "capture selection as new preset" (n from Preset).
    /// Stages the name in `modal_input`.
    PresetNameNew,
    /// Confirmation prompt before "apply current preset to ALL agents".
    PresetConfirmAll,
    /// GlobalReadOnly edit attempt → "promote to project?" confirmation.
    GlobalEditPrompt,
    /// Phase 5.2: Commands pane (.opencode/commands/*.md listing).
    Commands,
    /// Phase 5.3: Providers pane — providers from merged config (read-only list).
    Providers,
    /// Phase 5.4: MCP servers pane — `mcp.<name>` toggles & edits.
    Mcp,
    /// Phase 5.5: Permissions pane — cycle ask/allow/deny (global + per-agent).
    Permissions,
    /// Phase 5.6: Process & Logs pane — supervised `opencode` status + tail.
    Process,
    /// Phase 5.7: Settings pane — theme, default_agent, autoupdate, share.
    Settings,
    /// Providers edit-input modal (Enter commits CST write).
    ProviderEdit,
    /// Settings edit-input modal (Enter commits CST write, bools cycle).
    SettingsEdit,
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

/// Focus inside modal pickers (Model picker / Preset picker).
/// `Input` locks every printable char to the filter text box; `List` unlocks
/// the navigation/action keys (j/k/n/d/A). Tab toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalFocus {
    Input,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    ToggleSelectCurrent,
    /// Mouse click on row N of the agent list: move cursor + toggle.
    ToggleSelectRow(usize),
    /// Mouse click on row N: move cursor without toggling.
    MoveCursorTo(usize),
    /// `a` in List — alias that correctly picks Select-All vs Deselect-All
    /// based on current state. Mirrors prior `toggle_all_action()` behavior.
    ToggleAllAlias,
    SelectAll,
    DeselectAll,
    OpenModelModal,
    /// Show staged edits as a unified diff (D key; List mode only).
    OpenDiff,
    CloseDiff,
    /// Discard all staged edits: reload dirty agents from disk, clearing dirty.
    DiscardChanges,
    /// Diff-mode vertical scroll.
    DiffScroll(i32),
    OpenForm,
    OpenPicker,
    /// Phase 4 Presets: open preset picker modal (P key, List mode).
    OpenPresets,
    /// Preset modal: typed char filters the preset list.
    PresetInput(char),
    PresetBackspace,
    /// Enter on highlighted preset: apply to selected + close.
    PresetAccept,
    /// Preset modal n: open name-entry modal (capture selection as preset).
    PresetNewStart,
    /// Enter on the name-entry modal: save selection as preset.
    PresetSaveNew,
    /// Preset modal d: delete highlighted preset.
    PresetDelete,
    /// Preset modal Shift+A: prompt before applying to ALL agents.
    PresetApplyAllStart,
    ConfirmAllYes,
    ConfirmAllNo,
    /// GlobalReadOnly edit requested (+/- on a `·global` row): confirms user
    /// intent. `Yes` removes the failed-write and re-runs edit marking the
    /// promotion as a project-level write.
    GlobalEditYes,
    GlobalEditNo,
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
    /// Re-run provider model fetch (R in List). Event loop respawns fetcher.
    RefetchModels,
    /// Tab in Picker/Preset: toggle Input <-> List focus.
    ToggleModalFocus,
    /// Enter on MainMenu row: open that pane.
    MainMenuSelect,
    /// Digit key on MainMenu: set cursor + open.
    MainMenuJump(usize),
    /// Esc on a leaf pane: return to MainMenu.
    BackToMenu,
    /// Process pane: scroll the buffered tail up/down.
    ProcessScroll(i32),
    /// Process pane: request lifecycle action (handled async by the loop).
    ProcessStart,
    ProcessKill,
    ProcessRestart,
    /// Settings pane: cycle boolean value at cursor (bool keys only).
    SettingsToggle,
    /// Settings pane: start inline edit of string value at cursor.
    SettingsEditStart,
    SettingsEditCommit,
    /// Providers pane: cycle edit-subfield on highlighted provider.
    ProviderEditStart(&'static str),
    ProviderEditCommit,
    /// Providers pane: delete highlighted provider via CST.
    ProviderDelete,
    /// Commands pane: open name-entry modal ("new command").
    CommandNewStart,
    /// Commands pane: create file + refresh list.
    CommandCreate,
    /// Commands pane: delete highlighted file.
    CommandDelete,
    /// MCP pane: toggle `enabled` on the highlighted server.
    McpToggle,
    /// MCP pane: flip `type` between local/remote on the highlighted server.
    McpToggleType,
    /// MCP pane: delete the highlighted server.
    McpDelete,
    /// MCP pane: start inline edit of command/url (AgentList pattern).
    McpEditStart,
    /// MCP pane: commit the inline edit buffer.
    McpEditCommit,
    /// Permissions pane: cycle global value at cursor ask->allow->deny->ask.
    PermCycle,
    /// Permissions pane: cycle per-agent override value at cursor.
    PermCycleAgent(usize),
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
    /// Focus lock for picker modals; see `ModalFocus`.
    pub modal_focus: ModalFocus,
    /// Number of in-flight provider fetches; >0 means catalog may still grow.
    pub fetch_pending: u32,
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
    /// Per-endpoint fetch report channel; drained by the event loop each tick.
    fetch_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    fetch_tx: tokio::sync::mpsc::UnboundedSender<String>,
    /// Per-endpoint dedup: lines already logged within this session. Prevents
    /// the same unreachable-URL log from spamming the footer every frame.
    fetch_logged_lines: std::collections::HashSet<String>,
    /// Guards against re-logging the same fetch merge repeatedly.
    fetch_logged: bool,
    /// Cached pre-save diff text (recomputed on open).
    pub diff_text: Option<String>,
    /// Vertical scroll offset for the diff modal (0 = top).
    pub diff_scroll: u16,
    /// Highlighted row in the Main Menu.
    pub mainmenu_cursor: usize,
    /// Phase 4 presets: in-memory list from `.ocoger/presets.jsonc`.
    pub presets: Vec<Preset>,
    /// Live filter of `presets` (recomputed on `PresetInput`).
    pub preset_items: Vec<Preset>,
    /// Highlighted row in the preset modal.
    pub preset_cursor: usize,
    /// Preset selected for "apply to all" confirm; also holds the preset being
    /// captured while `Mode::PresetNameNew` is open so cancel can clean up.
    pub pending_preset: Option<Preset>,
    /// File handle for `.ocoger/presets.jsonc` (None if the file failed to load).
    presets_io: Option<Presets>,
    /// Resolved keybindings for the current session (defaults + global + project).
    pub keymap: Keymap,
    /// Resolved visual theme (from `theme` key of merged config).
    pub theme: Theme,
    /// Original (un-modified) label + step direction of the most recent edit
    /// on a `GlobalReadOnly` config entry. Set when user tries +/- on a
    /// `·global` row, consumed by `GlobalEditYes`.
    pending_global_edit: Option<(String, i32)>,
    /// Set by `Save` handling; `true` iff the last save wrote ≥1 file.
    last_save_wrote: bool,
    /// Phase 5.2: scanned `.opencode/commands/*.md` entries.
    pub commands: Vec<crate::core::commands::Command>,
    pub commands_cursor: usize,
    /// Reserved for when command create/delete lands (widget renders the flag).
    pub commands_is_dirty: bool,
    /// Phase 5.3: providers scanned once at boot from merged config.
    pub providers_list: Vec<crate::core::providers::ProviderInfo>,
    pub providers_cursor: usize,
    /// Reserved for when provider edit/delete lands (widget renders the flag).
    pub providers_is_dirty: bool,
    /// Phase 5.4: MCP servers snapshot from merged config (`mcp.<name>` map).
    pub mcp_list: Vec<crate::core::mcp::McpEntry>,
    pub mcp_cursor: usize,
    /// Phase 5.5: permission tuples — global + per-agent override values.
    pub perm_rows: Vec<crate::core::permissions::PermRow>,
    pub perm_cursor: usize,
    /// Phase 5.6: process & logs — buffered tail lines + lifecycle status.
    pub process_buf: Vec<String>,
    pub proc_state: crate::services::process_manager::ProcState,
    pub proc_pid: Option<u32>,
    pub proc_scroll: u16,
    /// Phase 5.7: settings rows — known top-level keys alongside current value.
    pub settings_rows: Vec<crate::core::settings::SettingsRow>,
    pub settings_cursor: usize,
    /// Modal-like staging for Providers edit (field index + buffer).
    providers_edit_field: Option<Vec<String>>,
    /// Pending command name inside CommandNewStart flow (held in modal_input).
    pub pending_command_name: Option<String>,
}

/// Phase 5 hub items. Index ↔ `mainmenu_cursor`. Binary digit keys (1..6)
/// jump directly. Implemented panes dispatch; unimplemented ones log a stub.
pub const MAINMENU_ITEMS: &[(&str, &str)] = &[
    (
        "Subagents",
        "edit .opencode/agents/*.md — models, params, presets",
    ),
    (
        "Providers & Models",
        "provider.baseURL / apiKey / headers + blacklist/whitelist",
    ),
    ("Permissions", "permission.<tool>: ask/allow/deny + globs"),
    (
        "MCP Servers",
        "mcp.<name>: Space toggle / t type / d delete",
    ),
    ("Commands", ".opencode/commands/*.md snippets — created!"),
    (
        "Process & Logs",
        "supervised opencode status + tail (Phase 5.6 partial)",
    ),
    ("Settings", "theme / default_agent / autoupdate / share"),
];

impl App {
    pub fn new(agents: Vec<AgentFile>, project_root: PathBuf) -> Self {
        let (config_items, config) = match config_resolver::ensure_loaded(&project_root) {
            // Resolver returns (editable-config, merged item list). Items are
            // already merged with GlobalReadOnly suffixes on foreign keys.
            Ok((c, items)) => (items, Some(c)),
            Err(e) => {
                let mut log = Vec::new();
                log.push(format!("[config] load failed: {e}"));
                (Vec::new(), None)
            }
        };
        // Boot with the static Anthropic catalog; live fetches merge in later
        // when the async refresh completes (services::model_fetcher).
        let picker_items: Vec<String> = crate::services::model_fetcher::ANTHROPIC_NATIVE_MODELS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (fetch_tx, fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let commands = crate::core::commands::list_commands(&project_root).unwrap_or_default();
        // Read-only panes (providers / mcp / permissions / settings) must show
        // the *effective* config, i.e. project + global merged. `config` is the
        // editable project file only, so it misses global-only keys entirely.
        let merged = crate::core::config_resolver::merged_value(&project_root);
        let config_value: Option<serde_json::Value> = if merged.is_object() {
            Some(merged)
        } else {
            config.as_ref().and_then(|c| c.value().ok())
        };
        let providers_list = config_value
            .clone()
            .and_then(|v| crate::core::providers::ProviderInfo::scan_providers(&v).ok())
            .unwrap_or_default();
        let mcp_list = config_value
            .as_ref()
            .map(|v| crate::core::mcp::McpEntry::scan(v))
            .unwrap_or_default();
        let agent_names: Vec<String> = agents
            .iter()
            .filter_map(|a| a.path.file_stem().map(|s| s.to_string_lossy().to_string()))
            .collect();
        let perm_rows = config_value
            .as_ref()
            .map(|v| crate::core::permissions::scan(v, &agent_names))
            .unwrap_or_else(|| crate::core::permissions::scan(&serde_json::Value::Null, &[]));
        let mut app = Self {
            agents,
            cursor: 0,
            should_quit: false,
            project_root,
            mode: Mode::MainMenu,
            modal_input: String::new(),
            modal_focus: ModalFocus::Input,
            fetch_pending: 0,
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
            fetch_rx,
            fetch_tx,
            fetch_logged_lines: std::collections::HashSet::new(),
            fetch_logged: false,
            diff_text: None,
            diff_scroll: 0,
            mainmenu_cursor: 0,
            presets: Vec::new(),
            preset_items: Vec::new(),
            preset_cursor: 0,
            pending_preset: None,
            presets_io: None,
            keymap: Keymap::defaults(),
            theme: Theme::default(),
            pending_global_edit: None,
            last_save_wrote: false,
            commands,
            commands_cursor: 0,
            commands_is_dirty: false,
            providers_list,
            providers_cursor: 0,
            providers_is_dirty: false,
            mcp_list,
            mcp_cursor: 0,
            perm_rows,
            perm_cursor: 0,
            process_buf: Vec::new(),
            proc_state: crate::services::process_manager::ProcState::Idle,
            proc_pid: None,
            proc_scroll: 0,
            settings_rows: config_value
                .as_ref()
                .map(|v| crate::core::settings::scan(v))
                .unwrap_or_default(),
            settings_cursor: 0,
            providers_edit_field: None,
            pending_command_name: None,
        };
        // Best-effort: load presets on boot; errors land in the log but never
        // block the UI (a corrupt presets file shouldn't break agent editing).
        if let Ok(ps) = Presets::load(&app.project_root) {
            app.presets = ps.items.clone();
            app.preset_items = ps.items.clone();
            app.presets_io = Some(ps);
        }
        // Resolve keymap: defaults -> global -> project,.warnings go to footer log.
        let (km, km_warnings) = Keymap::load(&app.project_root);
        app.keymap = km;
        for w in km_warnings {
            app.log(w);
        }
        // Theme: `theme` key from merged config; custom TOMLs under
        // .ocoger/themes/ take precedence over builtins of the same name.
        if let Some(cfg) = app.config.as_ref() {
            if let Ok(Some(name)) = cfg
                .value()
                .map(|v| v.get("theme").and_then(|t| t.as_str()).map(str::to_owned))
            {
                let themes_dir = app.project_root.join(".ocoger").join("themes");
                let (custom, twarn) = crate::ui::theme::load_custom_from(&themes_dir);
                for w in twarn {
                    app.log(format!("[theme] {w}"));
                }
                if let Some(t) = custom
                    .get(&name)
                    .copied()
                    .or_else(|| crate::ui::theme::by_name(&name))
                {
                    app.theme = t;
                } else {
                    app.log(format!(
                        "[theme] unknown '{name}'; using default (opencode)"
                    ));
                }
            }
        }
        app.spawn_catalog_fetch();
        app
    }

    /// Provider base URLs extracted from the loaded config for model fetch.
    fn provider_base_urls(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.config_items
            .iter()
            .filter(|i| i.label.ends_with(".base_url"))
            .map(|i| i.value.clone())
            .filter(|v| !v.is_empty())
            // Dedup identical URLs — otherwise the same endpoint gets
            // fetched twice and the log shows double entries.
            .filter(|v| seen.insert(v.clone()))
            .collect()
    }

    /// Spawn a background refresh; results feed `shared_catalog`.
    /// `fetch_pending` is bumped by the caller (or the event loop) so the UI
    /// can show a "loading" badge; the task decrements it via `fetch_tx`.
    pub fn spawn_catalog_fetch(&mut self) {
        use crate::services::model_fetcher;
        let urls = self.provider_base_urls();
        if urls.is_empty() {
            return;
        }
        self.fetch_pending = self.fetch_pending.saturating_add(urls.len() as u32);
        let shared = self.shared_catalog.clone();
        let report_tx = self.fetch_tx.clone();
        // Read API key pointer if exposed via config (env var name). Currently
        // heuristic (TODO Phase 2 polish): look for any *_api_key env name.
        let api_key_env = self
            .config_items
            .iter()
            .find(|i| i.label.ends_with(".api_key"))
            .map(|i| i.value.clone())
            .filter(|v| !v.is_empty());
        // Sync unit tests (cargo test) create `App` outside any Tokio runtime;
        // `tokio::spawn` panics in that context. Detect & skip — the live TUI
        // always runs inside #[tokio::main].
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        tokio::spawn(async move {
            let results = model_fetcher::refresh_catalog(urls, shared, api_key_env).await;
            for (base, r) in results {
                let line = match r {
                    Ok(n) => format!("[model-fetch] {base}: +{n} models"),
                    Err(e) => format!("[model-fetch ERROR] {base}: {e}"),
                };
                let _ = report_tx.send(line);
            }
            // Signal completion so the UI can clear the loading badge.
            let _ = report_tx.send("[model-fetch] ::done::".to_string());
        });
    }

    /// Merge shared live results into the picker-visible catalog (deduped,
    /// sorted) and drain the per-endpoint report channel into the log.
    pub fn sync_catalog_from_shared(&mut self) {
        while let Ok(line) = self.fetch_rx.try_recv() {
            if line == "[model-fetch] ::done::" {
                self.fetch_pending = 0;
                continue;
            }
            if self.fetch_logged_lines.insert(line.clone()) {
                self.log_push(line);
            }
        }
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
        if merged != self.picker_catalog {
            self.picker_catalog = merged;
            self.reload_picker_view();
        }
        if self.picker_catalog.len() != before && !self.fetch_logged {
            self.log(format!(
                "{} live model(s) merged into catalog",
                self.picker_catalog.len()
            ));
            self.fetch_logged = true;
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
        // Log every update for freeze diagnostics. Cheap: last mode+action only.
        tracing::debug!(mode = ?self.mode, action = ?action, "update");

        use Action::*;
        match action {
            MoveDown | MoveUp => match self.mode {
                Mode::MainMenu => Self::wrap(
                    &mut self.mainmenu_cursor,
                    MAINMENU_ITEMS.len(),
                    matches!(action, MoveDown),
                ),
                Mode::List => self.move_cursor(matches!(action, MoveDown)),
                Mode::Form => {
                    let n = self.form_item_count();
                    if n > 0 {
                        let next = matches!(action, MoveDown);
                        self.form_cursor = if next {
                            (self.form_cursor + 1) % n
                        } else {
                            (self.form_cursor + n - 1) % n
                        };
                    }
                }
                Mode::ModelEdit | Mode::Diff => {}
                Mode::Picker => {
                    let n = self.picker_items.len();
                    if n > 0 {
                        let next = matches!(action, MoveDown);
                        self.picker_cursor = if next {
                            (self.picker_cursor + 1) % n
                        } else {
                            (self.picker_cursor + n - 1) % n
                        };
                    }
                }
                Mode::Preset => {
                    let n = self.preset_items.len();
                    if n > 0 {
                        let next = matches!(action, MoveDown);
                        self.preset_cursor = if next {
                            (self.preset_cursor + 1) % n
                        } else {
                            (self.preset_cursor + n - 1) % n
                        };
                    }
                }
                Mode::PresetNameNew
                | Mode::PresetConfirmAll
                | Mode::GlobalEditPrompt
                | Mode::ProviderEdit
                | Mode::SettingsEdit => {}
                Mode::Commands => {
                    let n = self.commands.len();
                    if n > 0 {
                        Self::wrap(&mut self.commands_cursor, n, matches!(action, MoveDown));
                    }
                }
                Mode::Providers => {
                    let n = self.providers_list.len();
                    if n > 0 {
                        Self::wrap(&mut self.providers_cursor, n, matches!(action, MoveDown));
                    }
                }
                Mode::Mcp => {
                    let n = self.mcp_list.len();
                    if n > 0 {
                        Self::wrap(&mut self.mcp_cursor, n, matches!(action, MoveDown));
                    }
                }
                Mode::Permissions => {
                    let n = self.perm_rows.len();
                    if n > 0 {
                        Self::wrap(&mut self.perm_cursor, n, matches!(action, MoveDown));
                    }
                }
                Mode::Process => {}
                Mode::Settings => {
                    let n = self.settings_rows.len();
                    if n > 0 {
                        Self::wrap(&mut self.settings_cursor, n, matches!(action, MoveDown));
                    }
                }
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
                    self.modal_focus = ModalFocus::Input;
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
                if matches!(
                    self.mode,
                    Mode::ModelEdit | Mode::ProviderEdit | Mode::SettingsEdit
                ) {
                    self.modal_input.push(c);
                }
            }
            ModalBackspace => {
                if matches!(
                    self.mode,
                    Mode::ModelEdit | Mode::ProviderEdit | Mode::SettingsEdit
                ) {
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
            ToggleSelectRow(row) => {
                if self.mode == Mode::List && row < self.agents.len() {
                    self.cursor = row;
                    self.toggle();
                }
            }
            MoveCursorTo(row) => {
                if row < self.agents.len() {
                    self.cursor = row;
                }
            }
            ToggleAllAlias => {
                if self.mode == Mode::List {
                    let target = !(self.selected_count() == self.agents.len());
                    self.set_all(target);
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
            OpenDiff => {
                if self.mode == Mode::List && self.dirty_count() > 0 {
                    let diffs = crate::core::diff::agent_diffs(&self.agents, &self.project_root);
                    self.diff_text = Some(
                        diffs
                            .iter()
                            .map(|d| format!("--- {} (staged) ---\n{}", d.file_name, d.diff_text))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    self.diff_scroll = 0;
                    self.mode = Mode::Diff;
                } else {
                    self.log("nothing to diff (no staged changes)".to_string());
                }
            }
            DiffScroll(delta) => {
                if self.mode == Mode::Diff {
                    let max = self
                        .diff_text
                        .as_deref()
                        .map(|d| d.lines().count().saturating_sub(1) as u16)
                        .unwrap_or(0);
                    self.diff_scroll = if delta.is_negative() {
                        self.diff_scroll.saturating_sub(delta.unsigned_abs() as u16)
                    } else {
                        self.diff_scroll.saturating_add(delta as u16).min(max)
                    };
                }
            }
            DiscardChanges => {
                if self.mode == Mode::List {
                    let mut reloaded = 0;
                    let mut failed = 0;
                    for a in self.agents.iter_mut().filter(|a| a.is_dirty) {
                        match crate::core::agent_parser::load_agent(&a.path) {
                            Ok(fresh) => {
                                *a = fresh;
                                reloaded += 1;
                            }
                            Err(_) => failed += 1,
                        }
                    }
                    if failed > 0 {
                        self.log(format!(
                            "discard: reloaded {reloaded}, {failed} file(s) failed to re-read"
                        ));
                    } else if reloaded > 0 {
                        self.log(format!("discarded staged edits on {reloaded} agent(s)"));
                    } else {
                        self.log("no staged changes to discard".to_string());
                    }
                }
            }
            CloseDiff => {
                if self.mode == Mode::Diff {
                    self.diff_text = None;
                    self.diff_scroll = 0;
                    self.mode = Mode::List;
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
                // Discard staged input for ModelEdit / Picker / Preset* modes.
                if matches!(
                    self.mode,
                    Mode::ModelEdit
                        | Mode::Picker
                        | Mode::Preset
                        | Mode::PresetNameNew
                        | Mode::PresetConfirmAll
                        | Mode::ProviderEdit
                        | Mode::SettingsEdit
                ) {
                    // SettingsEdit cancel returns to Settings (mode marker, not row present).
                    let return_to_settings = self.mode == Mode::SettingsEdit;
                    self.modal_input.clear();
                    self.pending_preset = None;
                    self.modal_focus = ModalFocus::Input;
                    if self.pending_command_name.take().is_some() {
                        self.mode = Mode::Commands;
                    } else if self.providers_edit_field.is_some() {
                        self.mode = Mode::Providers;
                    } else if return_to_settings {
                        self.mode = Mode::Settings;
                    } else {
                        self.mode = Mode::List;
                    }
                }
            }
            OpenPresets => {
                if self.mode == Mode::List && !self.presets.is_empty() {
                    self.modal_input.clear();
                    self.preset_cursor = 0;
                    self.modal_focus = ModalFocus::Input;
                    self.mode = Mode::Preset;
                } else if self.mode == Mode::List {
                    self.log(
                        "No presets saved; save one first (no UI yet for creation from empty)"
                            .to_string(),
                    );
                }
            }
            PresetInput(c) => {
                if self.mode == Mode::Preset {
                    self.modal_input.push(c);
                    self.reload_preset_view();
                } else if self.mode == Mode::PresetNameNew {
                    self.modal_input.push(c);
                }
            }
            PresetBackspace => {
                if self.mode == Mode::Preset {
                    self.modal_input.pop();
                    self.reload_preset_view();
                } else if self.mode == Mode::PresetNameNew {
                    self.modal_input.pop();
                }
            }
            PresetAccept => {
                if self.mode == Mode::Preset {
                    self.preset_apply_to_selected();
                    self.mode = Mode::List;
                } else if self.mode == Mode::PresetConfirmAll {
                    // Enter on confirm = confirm
                    self.preset_apply_to_all_confirmed();
                    self.mode = Mode::List;
                }
            }
            PresetNewStart => {
                if self.mode == Mode::Preset {
                    // Require at least one selected agent to capture.
                    if self.selected_count() == 0 {
                        self.log(
                            "select agents first, then n to capture their settings".to_string(),
                        );
                    } else {
                        self.modal_input.clear();
                        self.mode = Mode::PresetNameNew;
                    }
                }
            }
            PresetSaveNew => {
                if self.mode == Mode::PresetNameNew {
                    // Disambiguate between "n" from the Preset picker (capture
                    // selection) and "n" from Commands pane (create file). The
                    // latter holds `pending_command_name`, so fork here.
                    if self.pending_command_name.is_some() {
                        self.command_create_from_modal();
                        // command_create sets Mode::Commands; don't overwrite
                    } else {
                        self.capture_selected_as_preset();
                        self.modal_input.clear();
                        self.mode = Mode::Preset;
                    }
                }
            }
            PresetDelete => {
                if self.mode == Mode::Preset {
                    if let Some(p) = self.preset_items.get(self.preset_cursor) {
                        let name = p.name.clone();
                        let mut save_err: Option<String> = None;
                        let mut ok = false;
                        if let Some(io) = self.presets_io.as_mut() {
                            if io.remove(&name) {
                                ok = true;
                                if let Err(e) = io.save() {
                                    save_err = Some(format!("preset delete save failed: {e}"));
                                }
                                self.presets = io.items.clone();
                            }
                        }
                        if ok {
                            self.reload_preset_view();
                            self.log(format!("preset '{name}' deleted"));
                        }
                        if let Some(e) = save_err {
                            self.log(e);
                        }
                    }
                }
            }
            PresetApplyAllStart => {
                if self.mode == Mode::Preset {
                    if let Some(p) = self.preset_items.get(self.preset_cursor).cloned() {
                        self.pending_preset = Some(p);
                        self.mode = Mode::PresetConfirmAll;
                    }
                }
            }
            ConfirmAllYes => {
                if self.mode == Mode::PresetConfirmAll {
                    self.preset_apply_to_all_confirmed();
                    self.mode = Mode::List;
                }
            }
            ConfirmAllNo => {
                if self.mode == Mode::PresetConfirmAll {
                    self.pending_preset = None;
                    self.mode = Mode::Preset;
                    self.log("apply-to-all cancelled".to_string());
                }
            }
            GlobalEditYes => {
                if self.mode == Mode::GlobalEditPrompt {
                    self.mode = Mode::Form;
                    // Re-run the edit path with promotion enabled: this is the
                    // same FormModify tick but on a fresh `pending_global_edit`
                    // (label captured when the prompt was raised).
                    if let Some((label, d)) = self.pending_global_edit.take() {
                        if let Some(err) = self.promote_global_to_project(&label, d) {
                            self.log(err);
                        } else {
                            self.log(format!("promoted '{label}' to project override"));
                        }
                    }
                }
            }
            GlobalEditNo => {
                if self.mode == Mode::GlobalEditPrompt {
                    self.pending_global_edit = None;
                    self.mode = Mode::Form;
                }
            }
            RefetchModels => {
                // Event loop performs the actual respawn (async). Here we only
                // clear dedup state so the fresh fetch logs again.
                if self.mode == Mode::List {
                    self.fetch_logged = false;
                    self.fetch_logged_lines.clear();
                    self.log("re-fetching model catalogs...".to_string());
                }
            }
            ToggleModalFocus => {
                if matches!(self.mode, Mode::Picker | Mode::Preset) {
                    self.modal_focus = match self.modal_focus {
                        ModalFocus::Input => ModalFocus::List,
                        ModalFocus::List => ModalFocus::Input,
                    };
                }
            }
            MainMenuSelect => self.mainmenu_apply(),
            MainMenuJump(i) => {
                if self.mode == Mode::MainMenu && i < MAINMENU_ITEMS.len() {
                    self.mainmenu_cursor = i;
                    self.mainmenu_apply();
                }
            }
            BackToMenu => {
                if self.mode != Mode::MainMenu {
                    self.mode = Mode::MainMenu;
                }
            }
            McpToggle => {
                if self.mode == Mode::Mcp {
                    self.mcp_toggle_current();
                }
            }
            McpToggleType => {
                if self.mode == Mode::Mcp {
                    self.mcp_toggle_type();
                }
            }
            McpDelete => {
                if self.mode == Mode::Mcp {
                    self.mcp_delete_current();
                }
            }
            McpEditStart | McpEditCommit => {
                self.log("inline command/url edit not implemented (Phase 5 polish)".to_string());
            }
            PermCycle => {
                if self.mode == Mode::Permissions {
                    self.perm_cycle(None);
                }
            }
            PermCycleAgent(agent_idx) => {
                if self.mode == Mode::Permissions {
                    self.perm_cycle(Some(agent_idx));
                }
            }
            ProcessScroll(d) => {
                if self.mode == Mode::Process {
                    let max = self.process_buf.len().saturating_sub(1) as u16;
                    self.proc_scroll = if d.is_negative() {
                        self.proc_scroll.saturating_sub(d.unsigned_abs() as u16)
                    } else {
                        self.proc_scroll.saturating_add(d as u16).min(max)
                    };
                }
            }
            ProcessStart | ProcessKill | ProcessRestart => {
                // Async side-effectful; event_loop performs and logs the result.
                if let Some(line) = self.process_buf.last().cloned() {
                    let _ = line;
                }
                self.log("process action requested (handled by event loop)".to_string());
            }
            SettingsToggle => {
                if self.mode == Mode::Settings {
                    self.settings_toggle_current();
                }
            }
            SettingsEditStart => {
                if self.mode == Mode::Settings {
                    self.settings_edit_current();
                }
            }
            SettingsEditCommit => {
                if self.mode == Mode::SettingsEdit {
                    let s = self.modal_input.trim().to_string();
                    if !s.is_empty() {
                        self.settings_edit_commit(&s);
                    }
                    self.modal_input.clear();
                    self.mode = Mode::Settings;
                }
            }
            ProviderEditStart(field) => {
                if self.mode == Mode::Providers {
                    self.providers_edit_start(field);
                }
            }
            ProviderEditCommit => {
                if self.mode == Mode::ProviderEdit {
                    self.providers_edit_commit();
                }
            }
            ProviderDelete => {
                if self.mode == Mode::Providers {
                    self.providers_delete_current();
                }
            }
            CommandNewStart => {
                if self.mode == Mode::Commands {
                    self.modal_input.clear();
                    self.mode = Mode::PresetNameNew; // reuse name-entry modal
                                                     // Note: PresetNameNew accepts will save as command below via branch.
                    self.pending_command_name = Some(String::new());
                }
            }
            CommandCreate => {
                if self.mode == Mode::PresetNameNew && self.pending_command_name.is_some() {
                    self.command_create_from_modal();
                }
            }
            CommandDelete => {
                if self.mode == Mode::Commands {
                    self.command_delete_current();
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

    /// Test helper: override boot mode without going through MainMenu.
    #[cfg(test)]
    pub fn with_mode(mut self, m: Mode) -> Self {
        self.mode = m;
        self
    }

    /// MainMenu Enter/digit: dispatch into the highlighted pane (stub-log until
    /// the pane ships — learner: only arm 0 (Subagents) is implemented in 5.1).
    fn mainmenu_apply(&mut self) {
        if self.mode != Mode::MainMenu {
            return;
        }
        match self.mainmenu_cursor {
            0 => self.mode = Mode::List,
            1 => self.mode = Mode::Providers,
            2 => self.mode = Mode::Permissions,
            3 => self.mode = Mode::Mcp,
            4 => self.mode = Mode::Commands,
            5 => self.mode = Mode::Process,
            6 => self.mode = Mode::Settings,
            i => {
                let name = MAINMENU_ITEMS.get(i).map(|(n, _)| *n).unwrap_or("?");
                self.log(format!("'{name}' pane not implemented yet (Phase 5)"));
            }
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
        // Read-only entries (sourced from the global config) trigger a
        // promotion flow: prompt for confirmation, and only if the user
        // accepts does the write go into the project file.
        if matches!(
            item.origin,
            crate::core::jsonc_config::ConfigOrigin::GlobalReadOnly
        ) {
            self.pending_global_edit = Some((item.label.clone(), d));
            self.mode = Mode::GlobalEditPrompt;
            return None;
        }
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

    /// Exposed for the confirm-modal render; label minus the `·global` suffix.
    pub fn pending_global_edit_label(&self) -> Option<String> {
        self.pending_global_edit
            .as_ref()
            .map(|(l, _)| l.trim_end_matches("·global").to_string())
    }

    /// Called after the user confirms a `GlobalReadOnly` promotion. Writes
    /// `new_val` into the project file for the original keypath.
    fn promote_global_to_project(&mut self, label: &str, d: i32) -> Option<String> {
        // Strip the `·global` decoration.
        let plain = label.trim_end_matches("·global");
        let item = self
            .config_items
            .iter()
            .find(|i| i.label.trim_end_matches("·global") == plain)?
            .clone();
        let cfg = self.config.as_mut()?;
        let cur: i32 = item.value.parse().unwrap_or(0);
        let new_val = (cur + d).to_string();
        if let Err(e) = cfg.set_nested_str(item.keypath.iter().map(|s| s.as_str()), &new_val) {
            return Some(format!("promote {plain} failed: {e}"));
        }
        // Refresh rows so the formerly-global row now shows as Primary.
        self.config_items = cfg.config_items().ok()?;
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

    // --- Phase 5.7 (Settings) helpers ---------------------------------------------

    /// Build the CST value for a settings row so each key lands in the file
    /// with its documented JSON type (bool / number / string).
    fn settings_cst_value(
        kind: crate::core::settings::Kind,
        value: &str,
    ) -> jsonc_parser::cst::CstInputValue {
        use crate::core::settings::Kind;
        use jsonc_parser::cst::CstInputValue as V;
        match kind {
            Kind::Bool => V::Bool(value == "true"),
            // `autoupdate` is a documented bool|"notify" union: keep the bools
            // typed and only "notify" as a string.
            Kind::BoolOrNotify => match value {
                "true" => V::Bool(true),
                "false" => V::Bool(false),
                other => V::String(other.to_owned()),
            },
            Kind::Number => match value.parse::<i64>() {
                Ok(n) => V::Number(n.to_string()),
                Err(_) => V::String(value.to_owned()),
            },
            Kind::Text | Kind::Enum(_) => V::String(value.to_owned()),
        }
    }

    fn settings_write(&mut self, value: &str) {
        let Some(row) = self.settings_rows.get(self.settings_cursor).cloned() else {
            return;
        };
        let cst = Self::settings_cst_value(row.kind, value);
        let Some(cfg) = self.config.as_mut() else {
            self.log("no writable config; cannot save".to_string());
            return;
        };
        match cfg.set_nested_value([&row.key], cst) {
            Ok(()) => match cfg.save() {
                Ok(()) => {
                    self.settings_rows[self.settings_cursor].value = value.to_string();
                    self.log(format!("{} = {}", row.key, value));
                }
                Err(e) => self.log(format!("save failed: {e}")),
            },
            Err(e) => self.log(format!("write failed: {e}")),
        }
    }

    /// Space on a settings row: flip bools, cycle documented enums. Free-text
    /// rows say so rather than silently doing nothing.
    fn settings_toggle_current(&mut self) {
        let Some(row) = self.settings_rows.get(self.settings_cursor).cloned() else {
            return;
        };
        match crate::core::settings::cycle(&row) {
            Some(next) => self.settings_write(&next),
            None => self.log(format!("'{}' is free text; press [e] to edit", row.key)),
        }
    }

    /// `e` / Enter: enums and bools cycle in place; text rows open the modal.
    fn settings_edit_current(&mut self) {
        let Some(row) = self.settings_rows.get(self.settings_cursor).cloned() else {
            return;
        };
        if !matches!(row.kind, crate::core::settings::Kind::Text) {
            self.settings_toggle_current();
            return;
        }
        self.modal_input = row.value.clone();
        self.mode = Mode::SettingsEdit;
        self.log(format!(
            "{}: {} — Enter commits, Esc cancels",
            row.key, row.hint
        ));
    }

    fn settings_edit_commit(&mut self, value: &str) {
        self.settings_write(value);
        self.settings_refresh();
    }

    /// Effective config (project + global merged) for read-only pane refresh.
    /// Falls back to the editable project file if nothing resolves.
    fn effective_config(&self) -> serde_json::Value {
        let merged = crate::core::config_resolver::merged_value(&self.project_root);
        if merged.is_object() {
            return merged;
        }
        self.config
            .as_ref()
            .and_then(|c| c.value().ok())
            .unwrap_or(serde_json::Value::Null)
    }

    fn settings_refresh(&mut self) {
        self.settings_rows = crate::core::settings::scan(&self.effective_config());
    }

    // --- Phase 5.4 (MCP) helpers ------------------------------------------------

    fn mcp_toggle_current(&mut self) {
        let Some(mut e) = self.mcp_list.get(self.mcp_cursor).cloned() else {
            return;
        };
        e.enabled = !e.enabled;
        let Some(cfg) = self.config.as_mut() else {
            return;
        };
        cfg.set_nested_value(
            ["mcp", &e.name, "enabled"],
            jsonc_parser::cst::CstInputValue::Bool(e.enabled),
        )
        .ok();
        let _ = cfg.save();
        self.mcp_list[self.mcp_cursor].enabled = e.enabled;
        self.log(format!(
            "mcp '{}' {}",
            e.name,
            if e.enabled { "enabled" } else { "disabled" }
        ));
    }

    fn mcp_toggle_type(&mut self) {
        let Some(e) = self.mcp_list.get(self.mcp_cursor).cloned() else {
            return;
        };
        let new_kind = if e.kind == "local" { "remote" } else { "local" };
        let Some(cfg) = self.config.as_mut() else {
            return;
        };
        if cfg
            .set_nested_str(["mcp", &e.name, "type"], new_kind)
            .is_ok()
        {
            let _ = cfg.save();
            self.mcp_list[self.mcp_cursor].kind = new_kind.to_string();
            self.log(format!("mcp '{}' type={}", e.name, new_kind));
        }
    }

    fn mcp_delete_current(&mut self) {
        let Some(e) = self.mcp_list.get(self.mcp_cursor).cloned() else {
            return;
        };
        let Some(cfg) = self.config.as_mut() else {
            return;
        };
        if cfg.remove_nested(["mcp", &e.name]).is_ok() {
            let _ = cfg.save();
            self.mcp_list.remove(self.mcp_cursor);
            if self.mcp_cursor > 0 {
                self.mcp_cursor -= 1;
            }
            self.log(format!("mcp '{}' deleted", e.name));
        }
    }

    // --- Phase 5.5 (Permissions) helpers ----------------------------------------

    fn perm_cycle(&mut self, agent_idx: Option<usize>) {
        let Some(row) = self.perm_rows.get(self.perm_cursor).cloned() else {
            return;
        };
        let (keypath, cur, label): (Vec<String>, String, String) = match agent_idx {
            None => (
                vec!["permission".to_string(), row.tool.clone()],
                row.global.clone(),
                format!("permission.{}", row.tool),
            ),
            Some(i) => {
                let Some(agent_name) = row.agent_overrides.keys().nth(i).cloned() else {
                    return;
                };
                let cur = row
                    .agent_overrides
                    .get(&agent_name)
                    .cloned()
                    .unwrap_or_default();
                (
                    vec![
                        "agent".to_string(),
                        agent_name.clone(),
                        "permission".to_string(),
                        row.tool.clone(),
                    ],
                    cur,
                    format!("agent.{}.permission.{}", agent_name, row.tool),
                )
            }
        };
        let next = crate::core::permissions::next_value(&cur);
        // A glob table (`"bash": { "rm *": "deny", ... }`) can't be replaced
        // with a bare verdict without silently deleting the user's rules.
        if cur.contains(" glob") || cur.contains('*') && cur.split_whitespace().count() > 1 {
            self.log(format!(
                "{label} holds pattern rules; edit them in the config file to avoid data loss"
            ));
            return;
        }
        let Some(cfg) = self.config.as_mut() else {
            return;
        };
        if cfg.set_nested_str(&keypath, next).is_ok() {
            let _ = cfg.save();
            self.perm_refresh();
            self.log(format!("{label} = {next}"));
        }
    }

    fn perm_refresh(&mut self) {
        let agent_names: Vec<String> = self
            .agents
            .iter()
            .filter_map(|a| a.path.file_stem().map(|s| s.to_string_lossy().to_string()))
            .collect();
        self.perm_rows = crate::core::permissions::scan(&self.effective_config(), &agent_names);
    }

    // --- Providers (5.3) edit/delete helpers ---------------------------------------

    /// Stage an edit on the highlighted provider: swap into the shared
    /// GlobalEditPrompt input shell so the user can type the new value.
    fn providers_edit_start(&mut self, field: &'static str) {
        let Some(p) = self.providers_list.get(self.providers_cursor).cloned() else {
            return;
        };
        let seg: Vec<&str> = field.split('.').collect();
        let mut path: Vec<String> = vec!["provider".to_string(), p.id];
        for s in seg {
            path.push(s.to_string());
        }
        self.providers_edit_field = Some(path);
        self.modal_input.clear();
        self.mode = Mode::ProviderEdit;
        self.log(format!(
            "type new value for {} — Enter commits, Esc cancels",
            field
        ));
    }

    /// Enter on the providers-input modal: CST write + refresh list.
    fn providers_edit_commit(&mut self) {
        let Some(path) = self.providers_edit_field.take() else {
            self.mode = Mode::Providers;
            return;
        };
        let value = self.modal_input.trim().to_string();
        let Some(cfg) = self.config.as_mut() else {
            self.mode = Mode::Providers;
            return;
        };
        if cfg.set_nested_str(&path, &value).is_ok() {
            let _ = cfg.save();
            self.providers_refresh();
            self.log(format!("provider.{} updated", path[1..].join(".")));
        }
        let _ = value;
        self.modal_input.clear();
        self.mode = Mode::Providers;
    }

    fn providers_delete_current(&mut self) {
        let Some(p) = self.providers_list.get(self.providers_cursor).cloned() else {
            return;
        };
        let Some(cfg) = self.config.as_mut() else {
            return;
        };
        if cfg.remove_nested(["provider", &p.id]).is_ok() {
            let _ = cfg.save();
            self.providers_list.remove(self.providers_cursor);
            if self.providers_cursor > 0 {
                self.providers_cursor -= 1;
            }
            self.log(format!("provider '{}' removed", p.id));
        }
    }

    fn providers_refresh(&mut self) {
        self.providers_list =
            crate::core::providers::ProviderInfo::scan_providers(&self.effective_config())
                .unwrap_or_default();
    }

    // --- Commands (5.2) create/delete -------------------------------------------------

    fn command_create_from_modal(&mut self) {
        let name = self.modal_input.trim().to_string();
        if name.is_empty() {
            self.log("empty command name; aborting".to_string());
            self.mode = Mode::Commands;
            self.pending_command_name = None;
            return;
        }
        let tmp = crate::core::commands::Command {
            name: name.clone(),
            description: "created via TUI".to_string(),
        };
        let body = format!("{}\n\n", tmp.serialize());
        let path = self
            .project_root
            .join(".opencode")
            .join("commands")
            .join(format!("{name}.md"));
        if let Err(e) = crate::core::fs_util::atomic_write(&path, &body) {
            self.log(format!("command create failed: {e}"));
        } else {
            self.log(format!("command '{name}' created"));
            if let Ok(cs) = crate::core::commands::list_commands(&self.project_root) {
                self.commands = cs;
                self.commands_cursor = self
                    .commands
                    .iter()
                    .position(|c| c.name == name)
                    .unwrap_or(0);
            }
        }
        self.mode = Mode::Commands;
        self.pending_command_name = None;
        self.modal_input.clear();
    }

    fn command_delete_current(&mut self) {
        let Some(c) = self.commands.get(self.commands_cursor).cloned() else {
            return;
        };
        let path = self
            .project_root
            .join(".opencode")
            .join("commands")
            .join(format!("{}.md", c.name));
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.log(format!("command '{}' removed", c.name));
                if let Ok(cs) = crate::core::commands::list_commands(&self.project_root) {
                    self.commands = cs;
                    if self.commands_cursor > 0 {
                        self.commands_cursor -= 1;
                    }
                }
            }
            Err(e) => self.log(format!("command delete failed: {e}")),
        }
    }

    fn wrap(cur: &mut usize, n: usize, next: bool) {
        if next {
            *cur = (*cur + 1) % n;
        } else {
            *cur = (*cur + n - 1) % n;
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

    fn reload_preset_view(&mut self) {
        let filter = self.modal_input.to_lowercase();
        self.preset_items = self
            .presets
            .iter()
            .filter(|p| {
                filter.is_empty()
                    || p.name.to_lowercase().contains(&filter)
                    || p.description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(&filter))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        if !self.preset_items.is_empty() && self.preset_cursor >= self.preset_items.len() {
            self.preset_cursor = self.preset_items.len() - 1;
        }
    }

    /// Apply the highlighted preset to all selected agents (`Enter` on Preset).
    fn preset_apply_to_selected(&mut self) {
        let Some(preset) = self.preset_items.get(self.preset_cursor).cloned() else {
            return;
        };
        let fields = preset.to_fields();
        let mut count = 0;
        for a in self.agents.iter_mut().filter(|a| a.is_selected) {
            a.update_models(&fields);
            count += 1;
        }
        self.log(format!(
            "preset '{}' applied to {count} selected agent(s) (s to save)",
            preset.name
        ));
    }

    /// Apply `pending_preset` to ALL agents (Shift+A confirmation arm).
    fn preset_apply_to_all_confirmed(&mut self) {
        let Some(preset) = self.pending_preset.take() else {
            return;
        };
        let fields = preset.to_fields();
        for a in self.agents.iter_mut() {
            a.update_models(&fields);
        }
        self.log(format!(
            "preset '{}' applied to ALL {} agent(s) (s to save)",
            preset.name,
            self.agents.len()
        ));
    }

    /// Capture settings from selected agents into a new preset named by
    /// `modal_input` (PresetNameNew modal), then persist.
    fn capture_selected_as_preset(&mut self) {
        let name = self.modal_input.trim().to_string();
        if name.is_empty() {
            self.log("preset name empty; cancelled".to_string());
            return;
        }
        // Aggregate: pick first-selected agent as the source of truth.
        let Some(src) = self.agents.iter().find(|a| a.is_selected) else {
            self.log("no selected agents to capture from".to_string());
            return;
        };
        let p = Preset {
            name: name.clone(),
            description: None,
            model: src.frontmatter.model.clone(),
            temperature: src.frontmatter.temperature,
            top_k: src.frontmatter.top_k,
            top_p: src.frontmatter.top_p,
            reasoning_effort: src.frontmatter.reasoning_effort.clone(),
        };
        if let Some(io) = self.presets_io.as_mut() {
            io.upsert(p.clone());
            match io.save() {
                Ok(()) => {
                    self.presets = io.items.clone();
                    self.reload_preset_view();
                    self.log(format!("preset '{name}' stored"));
                }
                Err(e) => self.log(format!("preset save failed: {e}")),
            }
        } else {
            // Lazy-initialize if initial load failed.
            if let Ok(ps) = Presets::load(&self.project_root) {
                self.presets = ps.items.clone();
                self.preset_items = ps.items.clone();
                self.presets_io = Some(ps);
            }
        }
    }

    fn save_dirty(&mut self) {
        let wrote = self.save_dirty_inner();
        self.last_save_wrote = wrote;
    }

    fn save_dirty_inner(&mut self) -> bool {
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
        failed.is_empty() && ok > 0
    }

    /// True iff the most recent `Save` action actually wrote ≥1 file.
    /// The event loop uses this to decide whether to restart the process —
    /// avoids re-saving (which would double-log) while preserving the
    /// PRD restart-after-save semantic.
    pub fn log_has_recent_save(&self) -> bool {
        self.last_save_wrote
    }

    /// Save all dirty agents, then signal whether the supervised process
    /// should be restarted (event loop observes this on `s` / `Ctrl+S`).
    /// Returns `true` only when at least one file was written.
    pub fn save_and_check_restart(&mut self) -> bool {
        self.save_dirty_inner()
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
            origin: crate::core::agent_parser::AgentOrigin::Project,
        }
    }

    fn app3() -> App {
        let mut a = App::new(vec![agent("a"), agent("b"), agent("c")], PathBuf::from("."));
        a.mode = Mode::List; // fixtures drive pane logic directly
        a
    }

    #[test]
    fn mainmenu_boot_dispatches_and_back_returns() {
        let mut app = App::new(vec![agent("a")], PathBuf::from("."));
        assert_eq!(app.mode, Mode::MainMenu, "boot mode is the hub");
        // Digit 1 = Subagents.
        app.update(Action::MainMenuJump(0));
        assert_eq!(app.mode, Mode::List);
        // Esc in a leaf pane returns to the hub.
        app.update(Action::BackToMenu);
        assert_eq!(app.mode, Mode::MainMenu);
        // Digit 2 = Permissions (was an unimplemented stub pre-Phase 5.5).
        app.update(Action::MoveDown);
        assert_eq!(app.mainmenu_cursor, 1);
        app.update(Action::MoveUp);
        assert_eq!(app.mainmenu_cursor, 0);
        // Providers (index 1) dispatches.
        app.update(Action::MainMenuJump(1));
        assert_eq!(app.mode, Mode::Providers);
        app.update(Action::BackToMenu);
        // Permissions (index 2) dispatches.
        app.update(Action::MainMenuJump(2));
        assert_eq!(app.mode, Mode::Permissions);
        app.update(Action::BackToMenu);
        // MCP (index 3) dispatches.
        app.update(Action::MainMenuJump(3));
        assert_eq!(app.mode, Mode::Mcp);
        app.update(Action::BackToMenu);
        assert_eq!(app.mode, Mode::MainMenu);
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
    fn settings_render_does_not_panic_on_empty_rows() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut app = App::new(vec![agent("a")], PathBuf::from("."));
        // Simulate Settings opened before any config is loaded (rows empty).
        app.settings_rows.clear();
        app.mode = Mode::Settings;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        // Frame that previously left raw mode active if a panic escaped.
        let res = terminal.draw(|f| {
            crate::ui::widgets::settings::render(f, f.area(), &app);
        });
        res.unwrap();
    }

    #[test]
    fn settings_mode_dispatches_edit_and_commit_round_trip() {
        let mut app = App::new(vec![agent("a")], PathBuf::from("."));
        // Enter Settings via MainMenu index 6 (boot hub = 0, then jump).
        app.mode = Mode::MainMenu;
        app.update(Action::MainMenuJump(6));
        assert_eq!(app.mode, Mode::Settings);
        // Enter opens the typed-input modal on the highlighted row.
        app.update(Action::SettingsEditStart);
        assert_eq!(app.mode, Mode::SettingsEdit);
        // Type then commit (theme is the first row).
        for ch in "dark".chars() {
            app.update(Action::ModalInput(ch));
        }
        app.update(Action::SettingsEditCommit);
        assert_eq!(app.mode, Mode::Settings);
        assert_eq!(
            app.settings_rows
                .iter()
                .find(|r| r.key == "theme")
                .map(|r| r.value.as_str()),
            Some("dark"),
            "commit persists to in-memory rows (CST write to disk applied on save)"
        );
        // Esc in edit mode returns to Settings.
        app.update(Action::SettingsEditStart);
        app.update(Action::CancelModal);
        assert_eq!(app.mode, Mode::Settings);
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
        app.mode = Mode::List;
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
    fn discard_changes_reloads_dirty_agents_from_disk() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-discard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.md");
        std::fs::write(&path, "---\nmodel: original\n---\nbody\n").unwrap();
        let mut app = App::new(
            vec![crate::core::agent_parser::load_agent(&path).unwrap()],
            dir,
        );
        app.mode = Mode::List;
        app.agents[0].is_selected = true;
        app.update(Action::OpenModelModal);
        for c in "tampered".chars() {
            app.update(Action::ModalInput(c));
        }
        app.update(Action::ApplyModelModal);
        assert_eq!(app.dirty_count(), 1);
        app.update(Action::DiscardChanges);
        assert_eq!(app.dirty_count(), 0);
        assert_eq!(app.agents[0].frontmatter.model, "original");
    }

    #[test]
    fn diff_scroll_clamps_and_resets_on_close() {
        let mut app = app3();
        app.agents[0].is_dirty = true;
        app.update(Action::OpenDiff);
        assert_eq!(app.mode, Mode::Diff);
        assert_eq!(app.diff_scroll, 0);
        app.update(Action::DiffScroll(-5));
        assert_eq!(app.diff_scroll, 0, "negative saturates at 0");
        app.update(Action::DiffScroll(2));
        assert_eq!(app.diff_scroll, 2);
        app.update(Action::CloseDiff);
        assert_eq!(app.diff_scroll, 0, "close resets scroll");
    }

    #[test]
    fn picker_jk_moves_cursor_with_wrap() {
        let mut app = app3();
        app.agents[0].is_selected = true;
        app.update(Action::OpenPicker);
        assert_eq!(app.mode, Mode::Picker);
        let n = app.picker_items.len();
        assert!(n >= 2, "static catalog has multiple entries");
        let start = app.picker_cursor;
        app.update(Action::MoveDown);
        assert_eq!(app.picker_cursor, (start + 1) % n);
        app.update(Action::MoveUp);
        assert_eq!(app.picker_cursor, start);
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
        app.mode = Mode::List;
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
