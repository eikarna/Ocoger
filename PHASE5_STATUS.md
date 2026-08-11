# Phase 5 Status — Hub-and-Spoke Architecture Progress

Date: 2026-08-11
Baseline: worktree after wiring fix (build green, 78 tests passing)

─────────────────────────────────────────────────────────────────────
Status Summary
─────────────────────────────────────────────────────────────────────
✓ Phase 5.1: MainMenu hub boot + navigation (complete)
✓ Phase 5.2: Commands pane (wired — read-only list + nav)
✓ Phase 5.3: Providers & Models pane (wired — read-only list + nav)
⏸ Phase 5.4+: Permissions/MCP/Process & Logs/Settings (not started)
✗ CRUD persistence for Commands/Providers (not implemented)

─────────────────────────────────────────────────────────────────────
Phase 5.1 — MainMenu Hub ✓ COMPLETE (commit 3d6457e)
─────────────────────────────────────────────────────────────────────
- Mode::MainMenu variant, MAINMENU_ITEMS (5 entries), digit shortcuts 1-5
- mainmenu_apply dispatch: 0→List, 1→Providers, 4→Commands, others log stub
- BackToMenu (Esc) from leaf panes; keymap + TOML overrides
- Test: mainmenu_boot_dispatches_and_back_returns

─────────────────────────────────────────────────────────────────────
Phase 5.2 — Commands Pane ✓ READ-ONLY FUNCTIONAL
─────────────────────────────────────────────────────────────────────
Important correction: commit 4407d6f ("Commands pane (5.2)") shipped
`core/commands.rs` + `widgets/commands.rs` but NEVER wired the pane —
no Mode variant, no App fields, no keymap, no render dispatch. The doc
previously claimed "complete"; that wiring landed in this worktree on
2026-08-11.

Current behavior:
- Boot scans `.opencode/commands/*.md` (name/description frontmatter)
  via `commands::list_commands`; missing dir → empty list, never blocks.
- MainMenu row 5 (index 4) → Commands pane; j/k/arrows navigate with
  wrap; Esc/q returns to MainMenu; mouse scroll works.
- Header shows count + dirty flag placeholder; hints text shows
  `[n] new  [d] delete` but those keys are NOT bound (no persistence).

Deferred (widget `commands_is_dirty` field is reserved for this):
- Create/delete on disk (needs name-entry modal like PresetNameNew,
  plus atomic write of the new .md file).
- Fix: `commands.rs::find_delimiters` indexes `content[y1 + 3..y2]` but
  `y1` is already the offset AFTER "---\n" — a pre-existing off-by-3 in
  parsing that makes most real files fail scanning. Fix with the wire-up.

─────────────────────────────────────────────────────────────────────
Phase 5.3 — Providers & Models Pane ✓ READ-ONLY FUNCTIONAL
─────────────────────────────────────────────────────────────────────
Wired 2026-08-11 (previously skeleton-only, build-broken).

Current behavior:
- Boot scans top-level `provider` map from the loaded opencode.json(c)
  (`ProviderInfo::scan_providers`): id, display name, options.baseURL,
  and whether apiKey contains an `{env:...}` reference.
- MainMenu row 2 (index 1) → Providers pane; j/k/arrows navigate with
  wrap; Esc/q returns to MainMenu; mouse scroll works.
- Entry shows name/baseURL/`[env:key]` marker. Read-only.

Deferred:
- Edit provider baseURL / apiKey via CST (`JsoncConfig::set_nested_str`
  exists — reuse the GlobalConfig form path, don't re-slice CST).
- Delete provider + confirm modal; persist via `config.save()`.
- blacklist/whitelist model filtering editor; per-provider headers.

─────────────────────────────────────────────────────────────────────
Phase 5.4+ — Deferred Work (not started)
─────────────────────────────────────────────────────────────────────
☐ Permissions pane (permission.<tool>: ask/allow/deny + globs)
☐ MCP Servers pane (mcp.<name>: enable/edit local|remote)
☐ Process & Logs pane (supervised opencode status + tail viewer)
☐ Settings pane (theme picker, default_agent, autoupdate, share)

─────────────────────────────────────────────────────────────────────
Build / Verification (2026-08-11)
─────────────────────────────────────────────────────────────────────
- `cargo test`: 78 passed, 0 failed (lib 70, integration 4 + 3 + 1-ignored)
- `cargo clippy`: no new warnings from Phase 5 wiring; 7 pre-existing
  warnings unrelated to panes (agent_parser, jsonc_config, form, etc.)
- `cargo fmt` applied.

Doc-drift notes:
- ARCH.md (previously "28 tests, zero warnings") corrected to 79.
- "Save/restart semantics only cover the Subagents pane" still true.
- `core/test_support.rs` was added by 5abbd57 but its `pub(crate) mod`
  declaration was lost from `core/mod.rs` at some point; restored here.
- `commands.rs::find_delimiters` off-by-3 (sliced `[y1+3..]`, dropping
  the first 3 bytes of every frontmatter) fixed; real files now parse.
