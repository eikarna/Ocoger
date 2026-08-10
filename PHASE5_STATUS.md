# Phase 5 Status — Hub-and-Spoke Architecture Progress

Date: 2026-08-10
Commit: 4407d6f (Phase 5.2 Commands pane complete)

─────────────────────────────────────────────────────────────────────
Status Summary
─────────────────────────────────────────────────────────────────────
✓ Phase 5.1: MainMenu hub boot + navigation (complete)
✓ Phase 5.2: Commands pane (complete) 
⚠ Phase 5.3: Providers & Models pane (skeleton created, not wired)
⏸ Phase 5.4+: Permissions/MCP/Process (not started)

─────────────────────────────────────────────────────────────────────
Phase 5.1 — MainMenu Hub ✓ COMPLETE (commit 3d6457e)
─────────────────────────────────────────────────────────────────────
- Mode::MainMenu enum variant
- MainMenuItems constants array (5 items, indices 0-4)
- mainmenu_apply() dispatching Pane modes on Enter/digit keys
- MoveDown/MoveUp handling cursor navigation with wraparound
- MainMenuSelect action + MainMenuJump(usize) digit shortcuts
- BackToMenu action returning from leaf panes to hub
- Main menu widget rendering numbered list with descriptions
- Keymap bindings: j/k/Enter/Tab/Q for MainMenu
- Test: mainmenu_boot_dispatches_and_back_returns

Status: Fully functional. Boot mode is now MainMenu instead of List.

─────────────────────────────────────────────────────────────────────
Phase 5.2 — Commands Pane ✓ COMPLETE (commit 4407d6f)
─────────────────────────────────────────────────────────────────────
Files created:
- src/core/commands.rs (Command struct, parse_from_frontmatter, list_commands)
- src/ui/widgets/commands.rs (render function)
- Updated src/core/mod.rs (pub mod commands)
- Updated src/ui/widgets/mod.rs (pub mod commands)

Struct changes (App):
- commands: Vec<crate::core::commands::Command>
- commands_cursor: usize
- commands_is_dirty: bool

Enum additions:
- Mode::Commands

Actions added:
- NewCommand (creates via name-entry modal — stub logging)
- DeleteCommand (removes from Vec, sets dirty flag — no persistence yet)
- OpenCommands (dispatches into Commands mode)

Wiring completed:
- render match in event_handler.rs (Mode::Commands => commands::render)
- move_down/move_up handlers in App::update()
- CancelModal handler returns to MainMenu
- Keymap defaults (Esc/j/k/n/d bindings for [Commands])
- TOML aliases recognized ('commands'/'cmd')

Test coverage:
- commands_pane_list_nav_delete (nav, new stub, delete, escape back)

Status: Functional stub pane. Commands scanned from .opencode/commands/*.md,
can navigate, create/delete log only (no disk persistence yet). Full CRUD UI
can be added by implementing create-form modal similar to ModelEdit modal.

─────────────────────────────────────────────────────────────────────
Phase 5.3 — Providers & Models Pane ⚠ PARTIAL (needs wiring)
─────────────────────────────────────────────────────────────────────
Files created:
- src/core/providers.rs (ProviderInfo struct, scan_providers method scanning opencode.json(c))
- src/ui/widgets/providers.rs (providers_render function)

Files updated:
- src/core/mod.rs (pub mod providers added)
- src/ui/widgets/mod.rs (pub mod providers added)

NOT YET IMPLEMENTED (blocking wiring):
1. ❌ Mode::Providers enum variant NOT ADDED to App enum
2. ❌ App struct fields MISSING:
   - providers_list: Vec<crate::core::providers::ProviderInfo>
   - providers_cursor: usize  
   - providers_is_dirty: bool
3. ❌ Actions NOT ADDED:
   - OpenProviders
   - NewProvider  
   - DeleteProvider
4. ❌ Render WIRING INCOMPLETE:
   - Providers render case present in event_handler.rs but NO MODE VARIANT
5. ❌ MoveDown/MoveUp HANDLER MISSING in App::update()
6. ❌ MainMenu dispatch INDEX 1 NOT ROUTING to Mode::Providers
7. ❌ CancelModal handler NOT RETURNING TO MAINMENU from Providers
8. ❌ Keymap DEFAULTS NOT SET for [Providers] mode
9. ❌ TOML ALIASES NOT DEFINED: "providers" or "provider" modes
10. ❌ INITIALIZATION NOT DONE in App::new():
    providers_list = ProviderInfo::scan_providers(&self.config).unwrap_or_default()
    providers_is_dirty = false

RECOMMENDED IMPLEMENTATION ORDER:
1. Add Mode::Providers enum variant BEFORE GlobalEditPrompt
2. Add App struct fields after providers_list declaration line
3. Add Actions enum variants near other Actions definitions
4. Wire initialization in App::new() before presets field
5. Wire MainMenu dispatch case index 1 → Mode::Providers
6. Wire MoveDown/MoveUp handlers in App::update()
7. Wire OpenProviders action handler (set mode, reset dirty flag)
8. Wire CancelModal handler for Providers → back to MainMenu
9. Wire render match case if not already there
10. Wire keymap defaults for [Providers] mode
11. Wire TOML aliases in Action::parse_mode_name()
12. Run cargo test --lib verifying compilation
13. Create basic test: providers_pane_list_nav_displays_items

STATUS CURRENT STATE:
- ProviderInfo model exists and can scan merged config
- Provider render widget exists displaying list + dirty indicator
- All skeleton code ready; ONLY missing wiring to app.rs enums/actions/handlers

FUTURE WORK (after wiring):
- Implement provider edit form (similar to Form band style)
- Implement delete confirmation dialog
- Persist provider changes via JsoncConfig CST editing (preserve comments)
- Add blacklist/whitelist model filtering editor
- Display per-provider settings (baseURL/apiKey/options.headers)

─────────────────────────────────────────────────────────────────────
Phase 5.4+ — Deferred Work (not yet started)
─────────────────────────────────────────────────────────────────────
☐ Permissions pane (permission.<tool>: ask/allow/deny + globs)
☐ MCP Servers pane (mcp.<name>: enable/edit local|remote)
☐ Process & Logs pane (supervised opencode status + tail viewer)
☐ Settings pane (theme picker, default_agent, autoupdate, share options)

─────────────────────────────────────────────────────────────────────
Immediate Next Steps
─────────────────────────────────────────────────────────────────────
Complete Phase 5.3 Wiring Tasks:
1. Add Mode::Providers enum variant
2. Add App struct fields (providers_list, providers_cursor, providers_is_dirty)
3. Add Actions (OpenProviders, NewProvider, DeleteProvider)
4. Wire all handlers (init, update(), keymap, render, dispatch)
5. Run tests verifying compilation
6. Commit Phase 5.3 completion

Then proceed:
- Phase 5.4 Permissions pane
- Phase 5.4 MCP Servers pane
- Phase 5.4 Process pane
- Testing polish across all panes
