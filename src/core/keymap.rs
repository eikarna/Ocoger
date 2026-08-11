//! User-tunable keybindings loaded from TOML (ROADMAP Phase 4 / TODO §5 P2).
//!
//! Sources (lowest precedence first):
//!   1. built-in defaults (hard-coded)
//!   2. `$XDG_CONFIG_HOME/ocoger/keymaps.toml` or `~/.config/ocoger/keymaps.toml`
//!   3. `<project>/.ocoger/tui_keymaps.toml`
//!
//! TOML shape is per-mode tables. Example:
//! ```toml
//! [list]
//! open_presets = "P"        # char literals are literal keys
//! open_picker  = "<S-p>"    # shift modifier on p
//!
//! [preset]
//! apply_all = "<S-a>"
//! ```
//!
//! Merge semantics: *per (mode, action)*. Specifying a new key for an action
//! overrides that single binding; everything else in the file falls through
//! to the next source. Conflicts (two actions bound to the same key in the
//! same mode) resolve deterministically: the later-loaded source wins; a
//! warning is surfaced.
//!
//! Parse/validation error philosophy (user decision): warn and skip the
//! invalid entry — never brick the UI because of a typo.

use crate::ui::app::{Action, Mode};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// A parsed key: code (crossterm's `KeyCode` abstraction) + modifier flags
/// (`ctrl`, `shift`, `alt`). Plain-char is the common path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub code: KeyCodeShape,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// We can't depend on crossterm inside the pure core. Keep a local enum
/// and translate at the event-handler boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCodeShape {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Space, // often clearer to write `space` than `' '`
}

#[derive(Debug, thiserror::Error)]
pub enum KeymapError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("toml parse: {0}")]
    Parse(String),
}

/// Keymap is a per-mode map from KeySpec -> Action. To keep this module
/// pure, Mode is only used as the lookup key.
#[derive(Debug, Clone)]
pub struct Keymap {
    pub tables: HashMap<Mode, HashMap<KeySpec, Action>>,
}

impl Keymap {
    /// Build the effective keymap: defaults -> global file -> project file.
    /// Returns the map plus warnings gathered from all sources.
    pub fn load(project_root: &Path) -> (Self, Vec<String>) {
        let mut cur = Self::defaults();
        let mut warnings = Vec::new();
        if let Some(p) = global_path() {
            cur.merge_file(&p, "global", &mut warnings);
        }
        cur.merge_file(
            &project_root.join(".ocoger").join("tui_keymaps.toml"),
            "project",
            &mut warnings,
        );
        (cur, warnings)
    }

    /// Boundaries that the default bindings live at — mirrors what
    /// `handle_key` originally hard-coded.
    pub fn defaults() -> Self {
        use KeyCodeShape::*;
        let def = |code, ctrl, shift, alt| KeySpec {
            code,
            ctrl,
            shift,
            alt,
        };
        let mut m: HashMap<Mode, HashMap<KeySpec, Action>> = HashMap::new();

        // ----- Main menu (Phase 5 hub) -----
        let mut t = HashMap::new();
        t.insert(def(Char('q'), false, false, false), Action::Quit);
        t.insert(def(Esc, false, false, false), Action::Quit);
        t.insert(def(Char('j'), false, false, false), Action::MoveDown);
        t.insert(def(Down, false, false, false), Action::MoveDown);
        t.insert(def(Char('k'), false, false, false), Action::MoveUp);
        t.insert(def(Up, false, false, false), Action::MoveUp);
        t.insert(def(Enter, false, false, false), Action::MainMenuSelect);
        // Digit shortcuts 1..=7 jump directly to a pane (one per menu item).
        for (i, ch) in ('1'..='7').enumerate() {
            t.insert(def(Char(ch), false, false, false), Action::MainMenuJump(i));
        }
        m.insert(Mode::MainMenu, t);

        // ----- List / global -----
        let mut t = HashMap::new();
        t.insert(def(Char('q'), false, false, false), Action::Quit);
        // Esc backs out to the Main Menu rather than quitting (hub pattern).
        t.insert(def(Esc, false, false, false), Action::BackToMenu);
        t.insert(def(Char('s'), false, false, false), Action::Save);
        t.insert(def(Char('r'), false, false, false), Action::Restart);
        t.insert(def(Char('R'), false, true, false), Action::RefetchModels);
        t.insert(def(Char('j'), false, false, false), Action::MoveDown);
        t.insert(def(Down, false, false, false), Action::MoveDown);
        t.insert(def(Char('k'), false, false, false), Action::MoveUp);
        t.insert(def(Up, false, false, false), Action::MoveUp);
        t.insert(def(Space, false, false, false), Action::ToggleSelectCurrent);
        t.insert(def(Enter, false, false, false), Action::ToggleSelectCurrent);
        t.insert(def(Char('a'), false, false, false), Action::ToggleAllAlias);
        t.insert(def(Char('m'), false, false, false), Action::OpenModelModal);
        t.insert(def(Char('p'), false, false, false), Action::OpenPicker);
        t.insert(def(Char('P'), false, true, false), Action::OpenPresets);
        t.insert(def(Char('d'), false, false, false), Action::OpenDiff);
        t.insert(def(Char('e'), false, false, false), Action::OpenForm);
        t.insert(def(Char('g'), false, false, false), Action::OpenForm);
        t.insert(def(Char('x'), false, false, false), Action::DiscardChanges);
        m.insert(Mode::List, t);

        // ----- Model edit modal (input buffer) -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Enter, false, false, false), Action::ApplyModelModal);
        m.insert(Mode::ModelEdit, t); // chars+backspace are pass-through

        // ----- Picker -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Enter, false, false, false), Action::PickerAccept);
        t.insert(def(Char('j'), false, false, false), Action::MoveDown);
        t.insert(def(Down, false, false, false), Action::MoveDown);
        t.insert(def(Char('k'), false, false, false), Action::MoveUp);
        t.insert(def(Up, false, false, false), Action::MoveUp);
        m.insert(Mode::Picker, t);

        // ----- Form -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::FormExit);
        t.insert(def(Char('q'), false, false, false), Action::FormExit);
        t.insert(def(Char('j'), false, false, false), Action::FormMove(true));
        t.insert(def(Down, false, false, false), Action::FormMove(true));
        t.insert(def(Char('k'), false, false, false), Action::FormMove(false));
        t.insert(def(Up, false, false, false), Action::FormMove(false));
        t.insert(def(Tab, false, false, false), Action::FormExit);
        t.insert(def(Char('e'), false, false, false), Action::FormExit);
        t.insert(def(Char('g'), false, false, false), Action::FormExit);
        t.insert(def(Char('+'), false, false, false), Action::FormModify(1));
        t.insert(def(Char('='), false, false, false), Action::FormModify(1));
        t.insert(def(Char('-'), false, false, false), Action::FormModify(-1));
        t.insert(def(Enter, false, false, false), Action::FormApply);
        m.insert(Mode::Form, t);

        // ----- Diff -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::CloseDiff);
        t.insert(def(Enter, false, false, false), Action::CloseDiff);
        t.insert(def(Char('j'), false, false, false), Action::DiffScroll(1));
        t.insert(def(Down, false, false, false), Action::DiffScroll(1));
        t.insert(def(Char('k'), false, false, false), Action::DiffScroll(-1));
        t.insert(def(Up, false, false, false), Action::DiffScroll(-1));
        m.insert(Mode::Diff, t);

        // ----- Preset -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Enter, false, false, false), Action::PresetAccept);
        t.insert(def(Char('n'), false, false, false), Action::PresetNewStart);
        t.insert(def(Char('d'), false, false, false), Action::PresetDelete);
        t.insert(def(Char('j'), false, false, false), Action::MoveDown);
        t.insert(def(Down, false, false, false), Action::MoveDown);
        t.insert(def(Char('k'), false, false, false), Action::MoveUp);
        t.insert(def(Up, false, false, false), Action::MoveUp);
        // Shift+A is "apply to all, with confirmation".
        t.insert(
            def(Char('a'), false, true, false),
            Action::PresetApplyAllStart,
        );
        m.insert(Mode::Preset, t);

        // ----- PresetNameNew -----
        let mut t = HashMap::new();
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Enter, false, false, false), Action::PresetSaveNew);
        m.insert(Mode::PresetNameNew, t);

        // ----- PresetConfirmAll -----
        let mut t = HashMap::new();
        t.insert(def(Char('y'), false, false, false), Action::ConfirmAllYes);
        t.insert(def(Char('Y'), false, false, false), Action::ConfirmAllYes);
        t.insert(def(Enter, false, false, false), Action::ConfirmAllYes);
        t.insert(def(Char('n'), false, false, false), Action::ConfirmAllNo);
        t.insert(def(Char('N'), false, false, false), Action::ConfirmAllNo);
        t.insert(def(Esc, false, false, false), Action::ConfirmAllNo);
        m.insert(Mode::PresetConfirmAll, t);

        // ----- GlobalEditPrompt -----
        let mut t = HashMap::new();
        t.insert(def(Char('y'), false, false, false), Action::GlobalEditYes);
        t.insert(def(Char('Y'), false, false, false), Action::GlobalEditYes);
        t.insert(def(Enter, false, false, false), Action::GlobalEditYes);
        t.insert(def(Char('n'), false, false, false), Action::GlobalEditNo);
        t.insert(def(Char('N'), false, false, false), Action::GlobalEditNo);
        t.insert(def(Esc, false, false, false), Action::GlobalEditNo);
        m.insert(Mode::GlobalEditPrompt, t);

        // ----- Providers edit: char input + Enter commit / Esc cancel -----
        let mut t = HashMap::new();
        t.insert(def(Enter, false, false, false), Action::ProviderEditCommit);
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Backspace, false, false, false), Action::ModalBackspace);
        m.insert(Mode::ProviderEdit, t);

        // ----- Settings edit: char input + Enter commit / Esc cancel -----
        let mut t = HashMap::new();
        t.insert(def(Enter, false, false, false), Action::SettingsEditCommit);
        t.insert(def(Esc, false, false, false), Action::CancelModal);
        t.insert(def(Backspace, false, false, false), Action::ModalBackspace);
        m.insert(Mode::SettingsEdit, t);

        // ----- Settings edit + Provider edit can bail via Ctrl+C too -----
        if let Some(tbl) = m.get_mut(&Mode::SettingsEdit) {
            tbl.insert(def(Char('c'), true, false, false), Action::Quit);
        }
        if let Some(tbl) = m.get_mut(&Mode::ProviderEdit) {
            tbl.insert(def(Char('c'), true, false, false), Action::Quit);
        }

        // ----- Commands / Providers / Permissions / MCP / Process / Settings leaf panes -----
        for mode in [
            Mode::Commands,
            Mode::Providers,
            Mode::Permissions,
            Mode::Mcp,
            Mode::Process,
        ] {
            let mut t = HashMap::new();
            t.insert(def(Esc, false, false, false), Action::BackToMenu);
            t.insert(def(Char('q'), false, false, false), Action::BackToMenu);
            t.insert(def(Char('j'), false, false, false), Action::MoveDown);
            t.insert(def(Down, false, false, false), Action::MoveDown);
            t.insert(def(Char('k'), false, false, false), Action::MoveUp);
            t.insert(def(Up, false, false, false), Action::MoveUp);
            match mode {
                Mode::Mcp => {
                    t.insert(def(Space, false, false, false), Action::McpToggle);
                    t.insert(def(Char('t'), false, false, false), Action::McpToggleType);
                    t.insert(def(Char('d'), false, false, false), Action::McpDelete);
                    t.insert(def(Char('e'), false, false, false), Action::McpEditStart);
                }
                Mode::Permissions => {
                    t.insert(def(Space, false, false, false), Action::PermCycle);
                    t.insert(
                        def(Char('e'), false, false, false),
                        Action::PermCycleAgent(0),
                    );
                }
                Mode::Providers => {
                    t.insert(
                        def(Char('e'), false, false, false),
                        Action::ProviderEditStart("options.baseURL"),
                    );
                    t.insert(
                        def(Char('k'), false, false, false),
                        Action::ProviderEditStart("options.apiKey"),
                    );
                    t.insert(def(Char('d'), false, false, false), Action::ProviderDelete);
                }
                Mode::Commands => {
                    t.insert(def(Char('n'), false, false, false), Action::CommandNewStart);
                    t.insert(def(Char('d'), false, false, false), Action::CommandDelete);
                }
                Mode::Process => {
                    t.insert(def(Char('s'), false, false, false), Action::ProcessStart);
                    // 'k' stays MoveUp (see base leaf bindings); kill gets 'x'.
                    t.insert(def(Char('x'), false, false, false), Action::ProcessKill);
                    t.insert(def(Char('r'), false, false, false), Action::ProcessRestart);
                }
                Mode::Settings => {
                    t.insert(def(Space, false, false, false), Action::SettingsToggle);
                    // Enter in Settings = open edit on the current row.
                    t.insert(def(Enter, false, false, false), Action::SettingsEditStart);
                    t.insert(
                        def(Char('e'), false, false, false),
                        Action::SettingsEditStart,
                    );
                }
                _ => {}
            }
            // Every leaf pane gets Ctrl+C → Quit, so raw-mode users can always bail.
            t.insert(def(Char('c'), true, false, false), Action::Quit);
            m.insert(mode, t);
        }

        Self { tables: m }
    }

    /// Apply a TOML source on top of `self`, appending warnings.
    fn merge_file(&mut self, path: &Path, src_name: &str, warnings: &mut Vec<String>) {
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => {
                warnings.push(format!(
                    "[keymap:{src_name}] read failed: {e}; keeping previous"
                ));
                return;
            }
        };
        let doc: toml::Value = match raw.parse::<toml::Table>() {
            Ok(t) => toml::Value::Table(t),
            Err(e) => {
                warnings.push(format!(
                    "[keymap:{src_name}] toml parse error: {e}; ignoring file"
                ));
                return;
            }
        };
        let Some(root) = doc.as_table() else { return };
        for (mode_name, v) in root {
            let Some(mode) = parse_mode(mode_name) else {
                warnings.push(format!(
                    "[keymap:{src_name}] unknown mode '{mode_name}'; skipping"
                ));
                continue;
            };
            let Some(tbl) = v.as_table() else {
                warnings.push(format!(
                    "[keymap:{src_name}] [{mode_name}] must be a table; skipping"
                ));
                continue;
            };
            for (action_name, key_value) in tbl {
                let Some(key_str) = key_value.as_str() else {
                    warnings.push(format!(
                        "[keymap:{src_name}] [{mode_name}].{action_name} must be a string; skipping"
                    ));
                    continue;
                };
                let Some(spec) = parse_key(key_str) else {
                    warnings.push(format!(
                        "[keymap:{src_name}] [{mode_name}].{action_name}: unparseable key '{key_str}'; skipping"
                    ));
                    continue;
                };
                let Some(action) = parse_action_in_mode(mode, action_name) else {
                    warnings.push(format!(
                        "[keymap:{src_name}] unknown action '{mode_name}::{action_name}'; skipping"
                    ));
                    continue;
                };
                let entry = self.tables.entry(mode).or_default();
                // Conflict detection: same key, different action in same mode.
                if let Some(prev) = entry.get(&spec) {
                    if *prev != action {
                        warnings.push(format!(
                            "[keymap:{src_name}] [{mode_name}] key '{key_str}' already bound to another action; {action_name} overrides"
                        ));
                    }
                }
                // Remove any prior keys bound to this action so rebinding replaces, not adds.
                entry.retain(|_, a| *a != action);
                entry.insert(spec, action);
            }
        }
    }

    /// Lookup bound action for this mode+key.
    pub fn lookup(&self, mode: Mode, key: KeySpec) -> Option<Action> {
        self.tables.get(&mode)?.get(&key).cloned()
    }
}

/// Accept natural-language mode names from TOML.
fn parse_mode(s: &str) -> Option<Mode> {
    match s {
        "main_menu" | "mainmenu" | "menu" => Some(Mode::MainMenu),
        "list" | "subagents" | "agents" => Some(Mode::List),
        "model_edit" | "modeledit" => Some(Mode::ModelEdit),
        "form" => Some(Mode::Form),
        "picker" => Some(Mode::Picker),
        "diff" => Some(Mode::Diff),
        "preset" => Some(Mode::Preset),
        "preset_name_new" | "preset_new" => Some(Mode::PresetNameNew),
        "preset_confirm_all" | "preset_confirm" => Some(Mode::PresetConfirmAll),
        "global_edit_prompt" => Some(Mode::GlobalEditPrompt),
        "commands" => Some(Mode::Commands),
        "providers" => Some(Mode::Providers),
        "providers_edit" => Some(Mode::ProviderEdit),
        "mcp" => Some(Mode::Mcp),
        "permissions" | "perms" => Some(Mode::Permissions),
        "process" => Some(Mode::Process),
        "settings" | "theme" => Some(Mode::Settings),
        _ => None,
    }
}

/// Accept `<S-a>`, `<C-x>`, `<A-x>`, `<C-S-x>`, plain chars (single letter or
/// symbol), plus named keys: `enter`, `esc`, `tab`, `space`, `backspace`,
/// `up`, `down`, `left`, `right`. Case: `<S-a>` = uppercase A, plain `a` =
/// lowercase.
fn parse_key(s: &str) -> Option<KeySpec> {
    use KeyCodeShape::*;
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('<').and_then(|x| x.strip_suffix('>')) {
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut last: &str = inner;
        // Strip "-"-separated prefixes.
        loop {
            if let Some(rest) = last.strip_prefix("C-").or_else(|| last.strip_prefix("c-")) {
                ctrl = true;
                last = rest;
            } else if let Some(rest) = last.strip_prefix("S-").or_else(|| last.strip_prefix("s-")) {
                shift = true;
                last = rest;
            } else if let Some(rest) = last.strip_prefix("A-").or_else(|| last.strip_prefix("a-")) {
                alt = true;
                last = rest;
            } else {
                break;
            }
        }
        let code = match last {
            "enter" | "Enter" => Enter,
            "esc" | "Esc" | "escape" => Esc,
            "tab" | "Tab" => Tab,
            "space" | "Space" => Space,
            "backspace" | "Backspace" | "bs" => Backspace,
            "up" | "Up" => Up,
            "down" | "Down" => Down,
            "left" | "Left" => Left,
            "right" | "Right" => Right,
            other => {
                let mut chars = other.chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None; // must be 1 char
                }
                Char(c)
            }
        };
        return Some(KeySpec {
            code,
            ctrl,
            shift,
            alt,
        });
    }
    // Bare key (1 char literal or named).
    match s {
        "enter" => Some(KeySpec {
            code: Enter,
            ctrl: false,
            shift: false,
            alt: false,
        }),
        "esc" | "escape" => Some(KeySpec {
            code: Esc,
            ctrl: false,
            shift: false,
            alt: false,
        }),
        "tab" => Some(KeySpec {
            code: Tab,
            ctrl: false,
            shift: false,
            alt: false,
        }),
        "space" => Some(KeySpec {
            code: Space,
            ctrl: false,
            shift: false,
            alt: false,
        }),
        "backspace" => Some(KeySpec {
            code: Backspace,
            ctrl: false,
            shift: false,
            alt: false,
        }),
        other => {
            let mut chars = other.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            // Bare uppercase char implies shift (crossterm reports 'P' with SHIFT).
            let shift = c.is_ascii_uppercase();
            Some(KeySpec {
                code: Char(c),
                ctrl: false,
                shift,
                alt: false,
            })
        }
    }
}

/// Map action names within a mode to Action values. Unknown → None.
/// Names are snake_case & stable across releases.
fn parse_action_in_mode(mode: Mode, name: &str) -> Option<Action> {
    use Mode::*;
    match (mode, name) {
        (MainMenu, "quit") => Some(Action::Quit),
        (MainMenu, "move_down") => Some(Action::MoveDown),
        (MainMenu, "move_up") => Some(Action::MoveUp),
        (MainMenu, "select") | (MainMenu, "open") => Some(Action::MainMenuSelect),
        (List, "quit") => Some(Action::Quit),
        (List, "back") | (List, "menu") => Some(Action::BackToMenu),
        (List, "save") => Some(Action::Save),
        (List, "restart") => Some(Action::Restart),
        (List, "refetch_models") => Some(Action::RefetchModels),
        (List, "move_down") => Some(Action::MoveDown),
        (List, "move_up") => Some(Action::MoveUp),
        (List, "select") | (List, "toggle") => Some(Action::ToggleSelectCurrent),
        (List, "toggle_all") => Some(Action::ToggleAllAlias),
        (List, "open_model_modal") => Some(Action::OpenModelModal),
        (List, "open_picker") => Some(Action::OpenPicker),
        (List, "open_presets") => Some(Action::OpenPresets),
        (List, "open_diff") => Some(Action::OpenDiff),
        (List, "open_form") => Some(Action::OpenForm),
        (List, "discard_changes") => Some(Action::DiscardChanges),

        (ModelEdit, "apply") => Some(Action::ApplyModelModal),
        (ModelEdit, "cancel") => Some(Action::CancelModal),

        (Picker, "accept") => Some(Action::PickerAccept),
        (Picker, "cancel") => Some(Action::CancelModal),
        (Picker, "move_down") => Some(Action::MoveDown),
        (Picker, "move_up") => Some(Action::MoveUp),

        (Form, "exit") => Some(Action::FormExit),
        (Form, "move_next") => Some(Action::FormMove(true)),
        (Form, "move_prev") => Some(Action::FormMove(false)),
        (Form, "modify_up") => Some(Action::FormModify(1)),
        (Form, "modify_down") => Some(Action::FormModify(-1)),
        (Form, "apply") => Some(Action::FormApply),

        (Diff, "close") => Some(Action::CloseDiff),
        (Diff, "scroll_down") => Some(Action::DiffScroll(1)),
        (Diff, "scroll_up") => Some(Action::DiffScroll(-1)),

        (Preset, "accept") => Some(Action::PresetAccept),
        (Preset, "new_from_selection") => Some(Action::PresetNewStart),
        (Preset, "delete") => Some(Action::PresetDelete),
        (Preset, "apply_all") => Some(Action::PresetApplyAllStart),
        (Preset, "cancel") => Some(Action::CancelModal),
        (Preset, "move_down") => Some(Action::MoveDown),
        (Preset, "move_up") => Some(Action::MoveUp),

        (PresetNameNew, "save") => Some(Action::PresetSaveNew),
        (PresetNameNew, "cancel") => Some(Action::CancelModal),

        (PresetConfirmAll, "yes") => Some(Action::ConfirmAllYes),
        (PresetConfirmAll, "no") => Some(Action::ConfirmAllNo),
        (GlobalEditPrompt, "yes") => Some(Action::GlobalEditYes),
        (GlobalEditPrompt, "no") => Some(Action::GlobalEditNo),
        _ => None,
    }
}

/// Global config path. `$XDG_CONFIG_HOME/ocoger/keymaps.toml`,
/// `$HOME/.config/ocoger/keymaps.toml` fallback.
fn global_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("ocoger").join("keymaps.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-keymap-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_keymap_contains_list_mode_basics() {
        let km = Keymap::defaults();
        let q = KeySpec {
            code: KeyCodeShape::Char('q'),
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(km.lookup(Mode::List, q), Some(Action::Quit));
        let big_p = KeySpec {
            code: KeyCodeShape::Char('P'),
            ctrl: false,
            shift: true,
            alt: false,
        };
        assert_eq!(km.lookup(Mode::List, big_p), Some(Action::OpenPresets));
    }

    #[test]
    fn parse_key_bare_char_and_named_and_modifier() {
        use KeyCodeShape::*;
        assert_eq!(
            parse_key("j"),
            Some(KeySpec {
                code: Char('j'),
                ctrl: false,
                shift: false,
                alt: false
            })
        );
        // Bare uppercase implies shift (crossterm reports SHIFT for '?').
        assert_eq!(
            parse_key("P"),
            Some(KeySpec {
                code: Char('P'),
                ctrl: false,
                shift: true,
                alt: false
            })
        );
        assert_eq!(
            parse_key("<C-s>"),
            Some(KeySpec {
                code: Char('s'),
                ctrl: true,
                shift: false,
                alt: false
            })
        );
        assert_eq!(
            parse_key("<A-d>"),
            Some(KeySpec {
                code: Char('d'),
                ctrl: false,
                shift: false,
                alt: true
            })
        );
        assert_eq!(
            parse_key("<C-S-x>"),
            Some(KeySpec {
                code: Char('x'),
                ctrl: true,
                shift: true,
                alt: false
            })
        );
        assert_eq!(
            parse_key("<Esc>"),
            Some(KeySpec {
                code: Esc,
                ctrl: false,
                shift: false,
                alt: false
            })
        );
        assert_eq!(
            parse_key("enter"),
            Some(KeySpec {
                code: Enter,
                ctrl: false,
                shift: false,
                alt: false
            })
        );
        // Bad input: multi-char bare.
        assert_eq!(parse_key("ab"), None);
        assert_eq!(parse_key("<C-S->"), None);
    }

    #[test]
    fn project_file_overrides_defaults() {
        let dir = temp_root("override");
        let ocoger = dir.join(".ocoger");
        fs::create_dir_all(&ocoger).unwrap();
        fs::write(
            ocoger.join("tui_keymaps.toml"),
            r#"
[list]
open_presets = "<C-p>"
"#,
        )
        .unwrap();

        // Note: load() also merges the global file. For determinism we bypass
        // load here and call merge_file on a fresh defaults map.
        let mut km = Keymap::defaults();
        let mut warnings = Vec::new();
        km.merge_file(&ocoger.join("tui_keymaps.toml"), "test", &mut warnings);
        assert_eq!(warnings.len(), 0, "no warnings expected: {warnings:?}");

        let ctrl_p = KeySpec {
            code: KeyCodeShape::Char('p'),
            ctrl: true,
            shift: false,
            alt: false,
        };
        assert_eq!(km.lookup(Mode::List, ctrl_p), Some(Action::OpenPresets));
        // Old default binding for P (shift char) was replaced by `retain`.
        let old_p = KeySpec {
            code: KeyCodeShape::Char('P'),
            ctrl: false,
            shift: true,
            alt: false,
        };
        assert_eq!(km.lookup(Mode::List, old_p), None);
    }

    #[test]
    fn invalid_action_name_warns_and_skips() {
        let dir = temp_root("bad-action");
        let ocoger = dir.join(".ocoger");
        fs::create_dir_all(&ocoger).unwrap();
        fs::write(
            ocoger.join("tui_keymaps.toml"),
            r#"
[list]
not_an_action = "z"
"#,
        )
        .unwrap();
        let mut km = Keymap::defaults();
        let mut warnings = Vec::new();
        km.merge_file(&ocoger.join("tui_keymaps.toml"), "test", &mut warnings);
        assert!(warnings.iter().any(|w| w.contains("not_an_action")));
        // Other defaults intact.
        let q = KeySpec {
            code: KeyCodeShape::Char('q'),
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(km.lookup(Mode::List, q), Some(Action::Quit));
    }

    #[test]
    fn unknown_mode_warns_and_skips() {
        let dir = temp_root("bad-mode");
        let ocoger = dir.join(".ocoger");
        fs::create_dir_all(&ocoger).unwrap();
        fs::write(
            ocoger.join("tui_keymaps.toml"),
            r#"
[not_a_mode]
quit = "z"
"#,
        )
        .unwrap();
        let mut km = Keymap::defaults();
        let mut warnings = Vec::new();
        km.merge_file(&ocoger.join("tui_keymaps.toml"), "test", &mut warnings);
        assert!(warnings.iter().any(|w| w.contains("not_a_mode")));
    }

    #[test]
    fn conflict_detected_and_later_wins() {
        let dir = temp_root("conflict");
        let ocoger = dir.join(".ocoger");
        fs::create_dir_all(&ocoger).unwrap();
        fs::write(
            ocoger.join("tui_keymaps.toml"),
            r#"
[list]
open_picker   = "o"
open_presets  = "o"
"#,
        )
        .unwrap();
        let mut km = Keymap::defaults();
        let mut warnings = Vec::new();
        km.merge_file(&ocoger.join("tui_keymaps.toml"), "test", &mut warnings);
        assert!(warnings
            .iter()
            .any(|w| w.contains("already bound") || w.contains("overrides")));
        let o = KeySpec {
            code: KeyCodeShape::Char('o'),
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(
            km.lookup(Mode::List, o),
            Some(Action::OpenPresets),
            "second binding wins"
        );
    }
}
