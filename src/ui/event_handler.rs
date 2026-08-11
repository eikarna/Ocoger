//! Crossterm event loop: input -> `Action` -> `App::update` -> render.

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseButton, MouseEventKind,
};
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
use crate::ui::widgets::{
    agent_list, commands, diff_view, form, mainmenu, mcp, permissions, picker, preset_picker,
    process, providers, settings,
};

/// Run the TUI until the user quits. Restores the terminal on all exits.
pub async fn run(mut app: App) -> io::Result<()> {
    // A panic inside `render` while raw mode is engaged leaves the terminal
    // hijacked: cursor invisible, Ctrl+C dead. Restore the terminal from a
    // panic hook so users can see the panic message instead of a frozen screen.
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        orig_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // Mouse capture: crossterm translates term escape-sequences into
    // `Event::Mouse`. Supported natively by Windows Terminal/CMD/PWSH,
    // Alacritty, WezTerm, Termux, Konsole, GNOME Terminal, iTerm2, Appel
    // Terminal (SGR 1006).
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    // Alternate screen inherits the terminal's scrollback. Without this, the
    // ratatui diff renderer believes the screen is already blank and skips
    // cells no widget paints, leaving old text visible under sparse panes.
    terminal.clear()?;

    let mut proc_mgr = ProcessManager::new();
    let result = event_loop(&mut app, &mut terminal, &mut proc_mgr).await;
    proc_mgr.shutdown_sync();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
        app.proc_state = proc_mgr.state;
        app.proc_pid = proc_mgr.pid;
        for (stream, line) in proc_mgr.drain_output() {
            let tagged = format!("[{stream}] {line}");
            app.log_push(tagged.clone());
            app.process_buf.push(tagged);
            if app.process_buf.len() > 500 {
                let drop = app.process_buf.len() - 500;
                app.process_buf.drain(..drop);
            }
        }
        if proc_mgr.state != last_proc_state {
            app.log_push(format!(
                "process state: {:?} -> {:?}",
                last_proc_state, proc_mgr.state
            ));
            last_proc_state = proc_mgr.state;
        }

        terminal.draw(|f| {
            use ratatui::style::{Modifier, Style};
            use ratatui::text::{Line, Span};
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
            let mode_style = match app.mode {
                Mode::MainMenu => Style::default().fg(app.theme.accent),
                Mode::List => Style::default().fg(app.theme.accent),
                Mode::ModelEdit => Style::default().fg(app.theme.syntax_keyword),
                Mode::Form => Style::default().fg(app.theme.accent),
                Mode::Picker => Style::default().fg(app.theme.warn),
                Mode::Diff => Style::default().fg(app.theme.accent),
                Mode::Preset => Style::default().fg(app.theme.syntax_keyword),
                Mode::PresetNameNew => Style::default().fg(app.theme.warn),
                Mode::PresetConfirmAll => Style::default().fg(app.theme.warn),
                Mode::GlobalEditPrompt | Mode::ProviderEdit | Mode::SettingsEdit => {
                    Style::default().fg(app.theme.warn)
                }
                Mode::Commands
                | Mode::Providers
                | Mode::Mcp
                | Mode::Permissions
                | Mode::Process
                | Mode::Settings => Style::default().fg(app.theme.accent),
            };
            let dirty_style = if app.dirty_count() > 0 {
                Style::default()
                    .fg(app.theme.warn)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.dim)
            };
            let header = Line::from(vec![
                Span::styled(" ◆ ", Style::default().fg(app.theme.accent)),
                Span::styled(
                    "ocoger ",
                    Style::default()
                        .fg(app.theme.fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(":: "),
                Span::raw(format!("{} agent(s) ", app.agents.len())),
                Span::styled(
                    format!("· {} selected ", app.selected_count()),
                    Style::default().fg(app.theme.dim),
                ),
                Span::styled(format!("· {} unsaved ", app.dirty_count()), dirty_style),
                Span::styled(format!("· mode {:?}", app.mode), mode_style),
                Span::styled(" · proc ", Style::default().fg(app.theme.dim)),
                Span::raw(proc_lbl),
                Span::styled(
                    format!(" · theme {} ", app.theme.name()),
                    Style::default().fg(app.theme.dim),
                ),
            ]);
            f.render_widget(ratatui::widgets::Paragraph::new(header), chunks[0]);

            match app.mode {
                Mode::MainMenu => mainmenu::render(f, chunks[1], app),
                Mode::List | Mode::ModelEdit => {
                    agent_list::render(f, chunks[1], app);
                    if app.mode == Mode::ModelEdit {
                        agent_list::render_modal(f, f.area(), app);
                    }
                }
                Mode::Form => form::render(f, chunks[1], app),
                Mode::Picker => {
                    agent_list::render(f, chunks[1], app);
                    picker::render(f, f.area(), app);
                }
                Mode::Diff => {
                    diff_view::render(f, f.area(), app.diff_text.as_deref(), app.diff_scroll);
                }
                Mode::Preset | Mode::PresetNameNew | Mode::PresetConfirmAll => {
                    agent_list::render(f, chunks[1], app);
                    preset_picker::render(f, f.area(), app);
                }
                Mode::Commands => commands::render(f, chunks[1], app),
                Mode::Providers => providers::render(f, chunks[1], app),
                Mode::ProviderEdit => {
                    providers::render(f, chunks[1], app);
                    // Render the inline edit line as a bottom prompt bar.
                    let bar = Line::from(vec![
                        Span::styled("edit > ", Style::default().fg(app.theme.warn)),
                        Span::raw(&app.modal_input),
                    ]);
                    f.render_widget(
                        ratatui::widgets::Paragraph::new(bar),
                        ratatui::layout::Rect::new(0, f.area().height - 1, f.area().width, 1),
                    );
                }
                Mode::Mcp => mcp::render(f, chunks[1], app),
                Mode::Permissions => permissions::render(f, chunks[1], app),
                Mode::Process => process::render(f, chunks[1], app),
                Mode::Settings | Mode::SettingsEdit => {
                    settings::render(f, chunks[1], app);
                    if app.mode == Mode::SettingsEdit {
                        let bar = Line::from(vec![
                            Span::styled("edit > ", Style::default().fg(app.theme.warn)),
                            Span::raw(&app.modal_input),
                        ]);
                        f.render_widget(
                            ratatui::widgets::Paragraph::new(bar),
                            ratatui::layout::Rect::new(0, f.area().height - 1, f.area().width, 1),
                        );
                    }
                }
                Mode::GlobalEditPrompt => {
                    form::render(f, chunks[1], app);
                    // Rendered as a simple modal paragraph on top of the form.
                    let w = 70u16;
                    let h = 6u16;
                    let area = f.area();
                    let x = area.x + area.width.saturating_sub(w) / 2;
                    let y = area.y + area.height.saturating_sub(h) / 2;
                    let popup =
                        ratatui::layout::Rect::new(x, y, w.min(area.width), h.min(area.height));
                    let label = app
                        .pending_global_edit_label()
                        .unwrap_or_else(|| "(? )".to_string());
                    let text = ratatui::text::Line::from(vec![
                        ratatui::text::Span::raw("promote '"),
                        ratatui::text::Span::styled(
                            label,
                            ratatui::style::Style::default()
                                .fg(app.theme.warn)
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        ),
                        ratatui::text::Span::raw("' to project? [y] create override  [n] cancel"),
                    ]);
                    f.render_widget(ratatui::widgets::Clear, popup);
                    f.render_widget(
                        ratatui::widgets::Paragraph::new(text).block(
                            ratatui::widgets::Block::default()
                                .borders(ratatui::widgets::Borders::ALL)
                                .border_type(ratatui::widgets::BorderType::Rounded)
                                .title(" promote-global "),
                        ),
                        popup,
                    );
                }
            }

            agent_list::render_bottom(f, chunks[2], app);
        })?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    tracing::debug!(?key, mode = ?app.mode, "key event");
                    // Hard escape hatches, evaluated before the keymap so a bad
                    // table can never trap the user:
                    //   Ctrl+C  — conventional SIGINT substitute (raw mode eats
                    //             the real signal).
                    //   Alt+F4  — the Windows console hands this to the focused
                    //             app in raw mode instead of closing the window,
                    //             so we must honour it ourselves.
                    if key.kind == KeyEventKind::Press {
                        let ctrl = key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL);
                        let alt = key.modifiers.contains(crossterm::event::KeyModifiers::ALT);
                        if (ctrl && key.code == KeyCode::Char('c'))
                            || (alt && key.code == KeyCode::F(4))
                        {
                            tracing::debug!(?key, "hard exit");
                            return Ok(());
                        }
                    }
                    if let Some(action) = dispatch_key_if_press(app, key) {
                        tracing::debug!(?action, mode = ?app.mode, "action dispatched");
                        maybe_restart(proc_mgr, app, action).await;
                        if app.should_quit {
                            return Ok(());
                        }
                    }
                }
                Event::Mouse(m) => {
                    if let Some(action) = dispatch_mouse(app, m) {
                        app.update(action);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Translate crossterm mouse event into an `Action` for the list pane.
/// Geometry mirrors the draw path: header(1) + content starts at y=1,
/// list block has a 1-row border + 1 header row before agent rows.
/// Returns None for events we don't bind (scroll, drag, move, right-click).
pub(crate) fn dispatch_mouse(app: &App, m: crossterm::event::MouseEvent) -> Option<Action> {
    // Scroll wheel: route to the active scrollable view.
    match m.kind {
        MouseEventKind::ScrollDown => {
            return match app.mode {
                Mode::Picker => Some(Action::MoveDown),
                Mode::Preset => Some(Action::MoveDown),
                Mode::Diff => Some(Action::DiffScroll(3)),
                Mode::List | Mode::Commands | Mode::Providers => Some(Action::MoveDown),
                _ => None,
            };
        }
        MouseEventKind::ScrollUp => {
            return match app.mode {
                Mode::Picker => Some(Action::MoveUp),
                Mode::Preset => Some(Action::MoveUp),
                Mode::Diff => Some(Action::DiffScroll(-3)),
                Mode::List | Mode::Commands | Mode::Providers => Some(Action::MoveUp),
                _ => None,
            };
        }
        _ => {}
    }
    if app.mode != Mode::List {
        return None;
    }
    let MouseEventKind::Down(MouseButton::Left) = m.kind else {
        return None;
    };
    // chunks[1] top = y=1 (after 1-line header). Border + header = 2 rows.
    const LIST_TOP: u16 = 1;
    const ROWS_ABOVE_AGENTS: u16 = 2; // block border + column header
    if m.row < LIST_TOP + ROWS_ABOVE_AGENTS {
        return None;
    }
    let row = (m.row - LIST_TOP - ROWS_ABOVE_AGENTS) as usize;
    if row >= app.agents.len() {
        return None;
    }
    // Same-row click toggles selection (mirrors Space); click elsewhere
    // moves the cursor so a second click / Space can toggle precisely.
    if row == app.cursor {
        Some(Action::ToggleSelectRow(row))
    } else {
        Some(Action::MoveCursorTo(row))
    }
}

/// Runs `handle_key` only when `key` is a Press event. Returns the action
/// dispatched (or None for Release/Repeat). Extracted so unit tests can drive
/// this pure predicate without spinning up a terminal.
pub(crate) fn dispatch_key_if_press(app: &mut App, key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    Some(handle_key(app, key))
}

/// If `action` is a process-restarting action (r / save with changes), perform
/// it and log the result.
async fn maybe_restart(proc_mgr: &mut ProcessManager, app: &mut App, action: Action) {
    use Action::*;
    if matches!(action, RefetchModels) {
        app.spawn_catalog_fetch();
        return;
    }
    let want_restart = match action {
        Restart => true,
        Action::ProcessRestart => true,
        // App::update(Save) already saved dirty agents; `log_has_recent_save`
        // reports whether that save actually wrote files (restart trigger).
        Save => app.log_has_recent_save(),
        Action::ProcessStart => {
            let cwd = app.project_root.clone();
            match proc_mgr.spawn(&cwd).await {
                Ok(pid) => app.log_push(format!("process spawned pid={pid}")),
                Err(e) => app.log_push(format!("spawn failed: {e}")),
            }
            return;
        }
        Action::ProcessKill => {
            match proc_mgr.kill().await {
                Ok(pid) => app.log_push(format!("process killed pid={pid}")),
                Err(e) => app.log_push(format!("kill failed: {e}")),
            }
            return;
        }
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
    // Focus lock: in Picker/Preset Input focus, printable chars go to the
    // filter *before* the keymap is consulted, so bound keys like j/k/n/d
    // still type. Tab flips focus. Non-char keys (arrows/Enter/Esc) fall
    // through to the keymap in either focus.
    let in_pickerish = matches!(app.mode, Mode::Picker | Mode::Preset);
    if in_pickerish && app.modal_focus == crate::ui::app::ModalFocus::Input {
        match key.code {
            KeyCode::Tab => {
                app.update(Action::ToggleModalFocus);
                return Action::ToggleModalFocus;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                let action = if app.mode == Mode::Picker {
                    Action::PickerInput(c)
                } else {
                    Action::PresetInput(c)
                };
                app.update(action.clone());
                return action;
            }
            _ => {} // fall through to keymap for arrows/Enter/Esc/Backspace
        }
    }
    handle_key_via_keymap(app, key)
}

/// Tab pressed while a picker modal is in List focus.
fn in_pickerish_list_focus(app: &App, key: KeyEvent) -> bool {
    matches!(app.mode, Mode::Picker | Mode::Preset)
        && app.modal_focus == crate::ui::app::ModalFocus::List
        && matches!(key.code, KeyCode::Tab)
}

fn handle_key_via_keymap(app: &mut App, key: KeyEvent) -> Action {
    use crate::core::keymap::{KeyCodeShape, KeySpec};
    // Translate crossterm KeyEvent into our pure KeySpec. Crossterm reports
    // SHIFT+A as `Char('A')` with SHIFT modifier; we canonicalize by
    // lowercasing when needed so "<S-a>" and "A" end up the same shape.
    let spec = match key.code {
        KeyCode::Char(' ') => KeySpec {
            code: KeyCodeShape::Space,
            ctrl: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
            shift: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT),
            alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
        },
        KeyCode::Char(c) => {
            // Crossterm on Windows/legacy terminals reports Shift+P as
            // Char('P') WITHOUT the SHIFT modifier flag. Mirror bare-char
            // keymap convention: an uppercase char implies shift so a
            // `<S-p>`-style binding still matches. Lowercase chars keep
            // shift=false.
            let implied_shift = c.is_ascii_uppercase();
            KeySpec {
                code: KeyCodeShape::Char(c),
                ctrl: key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL),
                shift: key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT)
                    || implied_shift,
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
        if let KeyCodeShape::Char(c) = spec.code {
            if c.is_ascii_uppercase() {
                // Retry with shift=true: user keymaps (and our default '<S-p>')
                // may bind the uppercase char without the shift flag.
                let shifted = spec_with(spec.code, spec.ctrl, true, spec.alt);
                app.keymap
                    .lookup(app.mode, shifted)
                    .or_else(|| {
                        let lower = spec_with(
                            KeyCodeShape::Char(c.to_ascii_lowercase()),
                            spec.ctrl,
                            true,
                            spec.alt,
                        );
                        app.keymap.lookup(app.mode, lower)
                    })
                    .or_else(|| {
                        let upper_no_shift = spec_with(spec.code, spec.ctrl, false, spec.alt);
                        app.keymap.lookup(app.mode, upper_no_shift)
                    })
            } else {
                // Typing 'a' may also match a user keymap entry `<S-a>` if
                // crossterm didn't pass SHIFT through.
                let shifted = spec_with(KeyCodeShape::Char(c), spec.ctrl, true, spec.alt);
                app.keymap.lookup(app.mode, shifted)
            }
        } else {
            None
        }
    });

    let action = match action {
        Some(a) => a,
        None => {
            // Tab in List focus flips back to Input.
            if in_pickerish_list_focus(app, key) {
                app.update(Action::ToggleModalFocus);
                return Action::ToggleModalFocus;
            }
            // Modal fallthroughs: these modes accept free-text input, so any
            // Char that isn't bound to an action still routes to the buffer.
            match (app.mode, key.code) {
                (Mode::ModelEdit, KeyCode::Char(c)) => Action::ModalInput(c),
                (Mode::ProviderEdit, KeyCode::Char(c)) => Action::ModalInput(c),
                (Mode::SettingsEdit, KeyCode::Char(c)) => Action::ModalInput(c),
                (Mode::Picker, KeyCode::Char(c)) => Action::PickerInput(c),
                (Mode::Preset, KeyCode::Char(c)) => Action::PresetInput(c),
                (Mode::PresetNameNew, KeyCode::Char(c)) => Action::PresetInput(c),
                (Mode::ModelEdit, KeyCode::Backspace) => Action::ModalBackspace,
                (Mode::ProviderEdit, KeyCode::Backspace) => Action::ModalBackspace,
                (Mode::SettingsEdit, KeyCode::Backspace) => Action::ModalBackspace,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_parser::{AgentFile, AgentFrontmatter};
    use crossterm::event::{KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn key_with_mods(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn scratch_app() -> App {
        let a = AgentFile {
            path: PathBuf::from("a.md"),
            frontmatter: AgentFrontmatter {
                model: "m".into(),
                temperature: None,
                top_k: None,
                top_p: None,
                reasoning_effort: None,
            },
            raw_body: String::new(),
            is_selected: true,
            is_dirty: false,
            raw_yaml: "model: m".into(),
            origin: crate::core::agent_parser::AgentOrigin::Project,
        };
        App::new(vec![a], PathBuf::from(".")).with_mode(Mode::List)
    }

    #[test]
    fn only_press_dispatches_action() {
        let mut app = scratch_app();
        // 'm' opens the ModelEdit modal only on Press.
        assert_eq!(
            dispatch_key_if_press(&mut app, key(KeyCode::Char('m'), KeyEventKind::Press)),
            Some(Action::OpenModelModal)
        );
        assert_eq!(app.mode, Mode::ModelEdit);

        // Release + Repeat of the same key must be filtered out.
        assert_eq!(
            dispatch_key_if_press(&mut app, key(KeyCode::Char('m'), KeyEventKind::Release)),
            None
        );
        assert_eq!(
            dispatch_key_if_press(&mut app, key(KeyCode::Char('m'), KeyEventKind::Repeat)),
            None
        );

        // Mode stays where Press left it — Release/Repeat must not "close and reopen".
        assert_eq!(
            app.mode,
            Mode::ModelEdit,
            "modal must remain open after Release"
        );
    }

    #[test]
    fn space_char_maps_to_space_shape_and_toggles_agent() {
        let mut app = scratch_app();
        assert!(app.agents[0].is_selected, "fixture starts selected");
        // Platform bug: crossterm reports Space as Char(' '); previously this
        // only matched a `Char(' ')` keymap entry which defaults don't have.
        dispatch_key_if_press(&mut app, key(KeyCode::Char(' '), KeyEventKind::Press));
        assert!(!app.agents[0].is_selected, "space must toggle selection");
    }

    #[test]
    fn shift_p_opens_presets_even_with_or_without_modifier_flag() {
        // Windows legacy path often reports Char('P') *without* SHIFT modifier.
        // Ensure both dispatches resolve to OpenPresets.
        let mut app = scratch_app();
        let no_shift = key_with_mods(KeyCode::Char('P'), KeyModifiers::NONE);
        let with_shift = key_with_mods(KeyCode::Char('P'), KeyModifiers::SHIFT);
        // List mode requires presets to exist; assert action id only.
        let a1 = dispatch_key_if_press(&mut app, no_shift).unwrap();
        let a2 = dispatch_key_if_press(&mut app, with_shift).unwrap();
        assert_eq!(a1, Action::OpenPresets, "bare 'P' (no shift flag)");
        assert_eq!(a2, Action::OpenPresets, "'P' + SHIFT");
    }

    #[test]
    fn lowercase_p_and_m_resolve_to_their_list_actions() {
        let mut app = scratch_app();
        let p = dispatch_key_if_press(&mut app, key(KeyCode::Char('p'), KeyEventKind::Press));
        // With zero selected agents, OpenPicker stays a no-op in update() —
        // but we expect the *dispatch* to identify the action correctly.
        assert_eq!(p, Some(Action::OpenPicker));
        // Fresh app for 'm' so the modal state from OpenPicker doesn't race
        // the assertion (typing 'm' inside a Picker would return PickerInput).
        let mut app2 = scratch_app();
        let m = dispatch_key_if_press(&mut app2, key(KeyCode::Char('m'), KeyEventKind::Press));
        assert_eq!(m, Some(Action::OpenModelModal));
    }

    #[test]
    fn mouse_left_down_toggles_clicked_row_when_cursor_is_there() {
        let mut app = scratch_app();
        // fixture cursor starts at 0, is_selected = true
        let ev = crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 1 + 2, // header + border + header-row
            modifiers: KeyModifiers::NONE,
        };
        let a = dispatch_mouse(&app, ev);
        assert_eq!(a, Some(Action::ToggleSelectRow(0)));
        app.update(a.unwrap());
        assert!(!app.agents[0].is_selected);
    }

    #[test]
    fn picker_input_focus_locks_j_k_to_filter_text() {
        let mut app = scratch_app();
        app.agents[0].is_selected = true;
        app.update(Action::OpenPicker);
        assert_eq!(app.mode, Mode::Picker);
        assert_eq!(app.modal_focus, crate::ui::app::ModalFocus::Input);
        // 'j' and 'k' must type into the filter, not move the cursor.
        let j = dispatch_key_if_press(&mut app, key(KeyCode::Char('j'), KeyEventKind::Press));
        let k = dispatch_key_if_press(&mut app, key(KeyCode::Char('k'), KeyEventKind::Press));
        assert_eq!(j, Some(Action::PickerInput('j')));
        assert_eq!(k, Some(Action::PickerInput('k')));
        assert_eq!(app.modal_input, "jk");
        assert_eq!(app.picker_cursor, 0, "cursor untouched while typing");
        // Arrows still navigate in Input focus.
        dispatch_key_if_press(&mut app, key(KeyCode::Down, KeyEventKind::Press));
        // Clear the filter so the list has rows to move through, then Tab
        // flips to List focus; now j/k navigate.
        app.modal_input.clear();
        app.update(Action::PickerBackspace); // no-op safe; triggers view reload below
        app.modal_input.clear();
        app.update(Action::ToggleModalFocus); // exercise update path too
        app.update(Action::ToggleModalFocus);
        dispatch_key_if_press(&mut app, key(KeyCode::Tab, KeyEventKind::Press));
        assert_eq!(app.modal_focus, crate::ui::app::ModalFocus::List);
        assert!(app.picker_items.len() > 1, "unfiltered catalog has rows");
        let before = app.picker_cursor;
        let jd = dispatch_key_if_press(&mut app, key(KeyCode::Char('j'), KeyEventKind::Press));
        assert_eq!(jd, Some(Action::MoveDown));
        assert_ne!(app.picker_cursor, before, "j moves cursor in List focus");
        // Tab back to Input.
        dispatch_key_if_press(&mut app, key(KeyCode::Tab, KeyEventKind::Press));
        assert_eq!(app.modal_focus, crate::ui::app::ModalFocus::Input);
    }

    #[test]
    fn refetch_models_dispatches_and_logs() {
        let mut app = scratch_app();
        let r = dispatch_key_if_press(
            &mut app,
            key_with_mods(KeyCode::Char('R'), KeyModifiers::SHIFT),
        );
        assert_eq!(r, Some(Action::RefetchModels));
        assert!(app.log.iter().any(|m| m.contains("re-fetching")));
    }

    #[test]
    fn mouse_scroll_routes_to_picker_and_diff() {
        let mut app = scratch_app();
        // Force picker mode with some items.
        app.agents[0].is_selected = true;
        app.update(Action::OpenPicker);
        assert_eq!(app.mode, Mode::Picker);
        let scroll_down = crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let a = dispatch_mouse(&app, scroll_down);
        assert_eq!(a, Some(Action::MoveDown), "picker scroll-down = MoveDown");
        let scroll_up = crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(dispatch_mouse(&app, scroll_up), Some(Action::MoveUp));
    }

    #[test]
    fn mouse_left_down_moves_cursor_when_clicking_other_row() {
        let mut app = scratch_app();
        // Clicks row 1 while cursor is 0 → expect cursor move, not toggle.
        let ev = crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 1 + 2 + 1,
            modifiers: KeyModifiers::NONE,
        };
        // Only one agent in fixture; dispatch must return None for out-of-range.
        let _ = dispatch_mouse(&app, ev);
        // Add a second agent to make row valid.
        app.agents.push(AgentFile {
            path: PathBuf::from("b.md"),
            frontmatter: crate::core::agent_parser::AgentFrontmatter {
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
        });
        let a = dispatch_mouse(&app, ev);
        assert_eq!(a, Some(Action::MoveCursorTo(1)));
        app.update(a.unwrap());
        assert_eq!(app.cursor, 1);
        assert!(
            !app.agents[1].is_selected,
            "cursor-move alone must not toggle"
        );
    }

    /// The edit modals are keyboard-driven text fields: unbound printable
    /// chars must reach the buffer, or the modal looks broken (typing does
    /// nothing). This regressed for ProviderEdit/SettingsEdit, which were
    /// missing from the fallthrough table.
    #[test]
    fn edit_modals_route_printable_chars_into_modal_input() {
        for mode in [Mode::ModelEdit, Mode::ProviderEdit, Mode::SettingsEdit] {
            let mut app = scratch_app();
            app.mode = mode;
            app.modal_input.clear();
            for ch in "abc".chars() {
                dispatch_key_if_press(&mut app, key(KeyCode::Char(ch), KeyEventKind::Press));
            }
            assert_eq!(app.modal_input, "abc", "{mode:?} must accept typed chars");
            dispatch_key_if_press(&mut app, key(KeyCode::Backspace, KeyEventKind::Press));
            assert_eq!(app.modal_input, "ab", "{mode:?} must accept Backspace");
        }
    }
}
