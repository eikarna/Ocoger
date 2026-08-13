# Product Roadmap (ROADMAP.md)
## Ocoger (OpenCode Manager)

---

## Roadmap Timeline Overview

```text
[ Phase 1: MVP Core ] --------> [ Phase 2: Provider Integration ] --------> [ Phase 3: Process Supervisor ] --------> [ Phase 4: Enterprise & Presets ] ---> [ Phase 5: Full Manager Surfaces ]
(Weeks 1-2) ✓ COMPLETE          (Weeks 3-4) ✓ COMPLETE                      (Weeks 5-6) ✓ COMPLETE                    (Weeks 7-8) ✓ COMPLETE             (Weeks 9-12) IN PROGRESS

```

---

## Phase 5: Full Manager Surfaces (Target: Weeks 9–12) — IN PROGRESS

Rationale: the app today manages *subagent frontmatter only*, but the name is "OpenCode Manager". Phase 5 promotes the shell to a hub that covers the full `opencode.json(c)` + `.opencode/` surface as documented at opencode.ai (config, agents, providers, permissions, MCP, commands, process). `MainMenu` mode becomes the entry point; the current agent list UI is preserved unchanged as one leaf pane.

* [x] **5.1 Main Menu shell.** `Mode::MainMenu` boot mode; `j/k` + digits 1-6 + Enter dispatch; `Esc` in a leaf pane returns. Current `List` kept as Subagents pane (enum name unchanged to avoid churn). Unimplemented panes log a stub.
* [x] **5.2 Commands pane.** `.opencode/commands/*.md` — scans name/description frontmatter; list + cursor nav + Esc/q back; create, edit (`description`/`agent`/`model`), and project-local delete persist atomically. Global commands remain read-only.
* [ ] **5.3 MCP servers pane.** Reads `mcp.*` from merged JSONC; Space toggles `enabled: bool` via CST; `e` opens edit form (type local|remote, command/url, env, headers); `n` adds a new entry.
* [ ] **5.4 Permissions pane.** Reads `permission.*` (global) and `agent.<name>.permission.*` (per-agent). Cycles `ask`/`allow`/`deny` on focus; `e` edits glob pattern tables (e.g., `bash` rules with `git *`/`npm *` last-match-wins semantics preserved by CST append order).
* [~] **5.5 Providers & Models pane.** List view DONE (scan `provider.<id>` from merged JSONC; show name / `options.baseURL` / `{env:VAR}` key marker; nav). Edit/delete via CST, headers, models table, blacklist/whitelist editors, `limit.context/output` NOT YET.
* [ ] **5.6 Process & Logs pane.** Promote `ProcessManager` from footer to full pane: state, pid, uptime, tail `[stdout]`/`[stderr]` with scroll, `R` restart, `K` kill, `S` start, `opencode serve` port config.
* [ ] **5.7 Settings/Theme pane.** `theme` picker (7 built-in + custom TOMLs), `default_agent`, `autoupdate`, `share` fields.

---

## Deferred (not Phase 5)

* Sessions/snapshots pane (opencode `snapshots` field is UI-internal; out of scope)
* LSP server configuration widgets (rarely touched; JSONC path suffices)
* Plugins pane (`plugin: []` array — trivial CST edit but a low-value standalone pane)


---

## Detailed Milestones

### Phase 1: MVP & Local Configuration Engine ✓ COMPLETE (all criteria met 2026-08-08)

* [x] **Project Scaffolding:** Cargo project initialized (`cargo init`, edition 2021, MSRV 1.75), module architecture set (`core/`, `services/`, `ui/`).
* [x] **Subagent Scanner:** `walkdir` scan for `.opencode/agents/*.md` (`src/core/agent_scanner.rs`). *(non-recursive for MVP; recursion later)*
* [x] **Frontmatter Parser:** `gray_matter` extracts/modifies agent YAML metadata (`src/core/agent_parser.rs`).
    * **Fidelity guarantee:** parse → serialize round-trip is **byte-identical** (Markdown body *and* raw YAML slices incl. comments + key order) — beyond PRD's body-only contract.
    * `ParsedAgent`/`AgentFile` pairs: raw YAML + body are stored verbatim; `AgentFile::update_models` / `set_model` regex-splice target values **without touching trailing ` # comment` anchors**.
* [x] **Basic TUI Interface:** Primary list view with multi-select (`Space`) via `ratatui`; cursor wraps, header shows selected/unsaved counts.
* [x] **Single & Batch Model Editor:** `m` opens modal; `Enter` applies to all selected agents; typed text + Backspace stage change live.
* [x] **Atomic File Saver:** `core::fs_util::atomic_write` (write `.tmp` → `fs::rename`). Used by both agent file persistence and JSONC config. Includes Windows-safe "overwrite existing" test.

### Phase 2: Provider API Discovery & JSONC Engine ✓ COMPLETE (all criteria met 2026-08-08)

* [x] **JSONC Parser Integration:** `jsonc-parser` **spiked 15/15** in `examples/jsonc_spike.rs` — CST edits are byte-surgical (100% of comments + trailing commas preserved). Replaces outdated `comment-json`/`serde_json5` from original ARCH sketch.
* [x] **Global Config UI:** Form mode (`e`/`g` to enter, `Tab` to switch panes, `←→` band switch) shows two bands: `Agent params` and `Global config (opencode.jsonc)`. Fields extracted from `opencode.jsonc`: `model`, `theme`, `default_provider`, and per-provider `base_url`/`api_key`/`options.base_url`. Edits via `+`/`-`/`Enter`; `j`/`k` move. Edits are surgical CST (`set_value`/`append`, never serde round-trip).
* [x] **Async Model Discovery Engine:** `services/model_fetcher.rs` spawns parallel `tokio::spawn(openai-fetch)` per configured `provider.base_url` with `reqwest`; results stream into App via `mpsc`, merged into shared `Arc<RwLock<HashSet<String>>>`. Includes a `fetch()` timeout → per-endpoint error now shows in the log `[model-fetch] N models (ms)` or `[model-fetch] err=… (ms)`.
* [x] **Static Fallback Catalog:** `ANTHROPIC_NATIVE_MODELS` → initial `picker_catalog` merges fetched results into; catalog is **deduped + sorted**; stale entries preserved on fetch errors (no clobbering).
* [x] **Interactive Model Picker Modal:** `p` opens the picker; live fuzzy filter via typing (`picker_items` computed every keystroke), `Enter` stages model on selected agents, `Esc`/`Cancel` closes. Visual cursor follows filtered results.

### Phase 3: Process Supervision & Live Logging ✓ COMPLETE (all criteria met 2026-08-08)

* [x] **Process Manager Engine:** `services/process_manager.rs` wraps `tokio::process` spawn/kill/restart, with OS-aware lookup via `cmd`/`opencode.cmd` on Windows vs `opencode` on Unix (`where`-style `find_executable`).
* [x] **Signal Controller:** Windows-aware kill; `spawn` drops on Drop, `restart` spawns after 3s wait; `kill` uses 3s `wait_timeout`. Verified by stub test `kill_clears_state_and_returns_pid`.
* [x] **Hot-Restart Automation:** `Ctrl+S` hotkey runs `save_and_check_restart()` — writes dirty agents via atomic write, then issues `proc_mgr.restart()` only when files actually changed (save-only-on-change logic).
* [x] **Live Terminal Output Drawer:** stdout/stderr piped via `tokio::process` pipes; lines read in tasks (`mpsc`) then appended to App `log` via each tick drain — visible in the footer `[stdout]`/`[stderr]`.

### Phase 4: Presets, Profiles & Polish (Target: Week 8) — ✓ COMPLETE (2026-08-10)

* [x] **Configuration Presets:** `.ocoger/presets.jsonc` — capture from selection (`n`), apply to selected (`Enter`), apply-to-all with confirm (`Shift+A` → y/n), delete (`d`), live filter (focus-locked).
* [x] **Diff Previewer Modal:** unified pre-save diff via `Mode::Diff`; multi-file concatenation + `j/k`/wheel scroll.
* [x] **Custom Hotkey Rebinding:** `~/.config/ocoger/keymaps.toml` (XDG) → `<project>/.ocoger/tui_keymaps.toml`, per-(mode, action) merge with conflict warnings.
* [x] **Binary Packaging:** GitHub Actions release matrix across 8 targets (win x64/arm64, linux gnu/musl x64/arm64 incl. Alpine/Termux, macOS x64/arm64); musl static + cross-built intel-mac on arm64 runner.
* [x] **First-class input UX fixes:** Space/Enter select, Shift+P, single-save logging, click-to-toggle mouse, focus-locked picker filter (Tab to switch Input↔List), fetch-pending badge, `R` re-fetch, discard-staged (`x`).
* [x] **One-line installers:** `install.sh` (POSIX) + `install.ps1` (Windows), detecting OS/arch and deploying to user-local bin with PATH bootstrap.

---

## Session Notes (2026-08-10)

Phase 4 marked complete. Phase 5 scoped in: promote the shell to a hub covering the full opencode config surface, starting from a new `Mode::MainMenu`. Current agent list UI is retained as the Subagents leaf pane, unchanged.

**Near-term priorities:** 5.1 MainMenu scaffolding (one new mode + dispatch), then 5.2 Commands pane (near-free reuse of `agent_parser`/`agent_scanner`), then 5.3 MCP toggle via CST as the first non-agent module proving `jsonc_config` generalizes.
