# Product Roadmap (ROADMAP.md)
## Ocoger (OpenCode Manager)

---

## Roadmap Timeline Overview

```text
[ Phase 1: MVP Core ] --------> [ Phase 2: Provider Integration ] --------> [ Phase 3: Process Supervisor ] --------> [ Phase 4: Enterprise & Presets ]
(Weeks 1-2) ✓ COMPLETE          (Weeks 3-4) ✓ COMPLETE                      (Weeks 5-6) ✓ COMPLETE                    (Weeks 7-8) IN PROGRESS

```

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

### Phase 4: Presets, Profiles & Polish (Target: Week 8) — IN PROGRESS

* [ ] **Configuration Presets:** Save/Load profile snapshots (e.g., "Deep Work / Reasoning Mode", "Fast Draft / Cheap Mode").
* [x] **Diff Previewer Modal:** unified pre-save diff preview via `Mode::Diff`; staged edits write to `.ocoger/staging/…` (atomic), diff computed per changed agent via `similar` text diff; D key on Any mode (List/Model/Form/Picker all) for review before application. Handles dirty tracking and per-file memoization.
* [ ] **Custom Hotkey Rebinding:** Configurable keybindings file (`tui_keymaps.toml`).
* [ ] **Binary Packaging:** Release pipeline for GitHub Actions producing prebuilt binaries for Linux, macOS, and Windows.

---

## Session Notes (2026-08-08)

Phase 1+2+3 MVP features are functional and test-covered (28 simple integration-ish unit tests in `core::`-pedagogy: parser, scanner; jsonc config; file utilities; `ui::App` model with `dirty`, last message state; services process manager). The TUI supports:

* batch/hot-edit (`m`, `+`, `-`, `e`, `g`)
* process supervision (`Ctrl+S` → restart → timestamps queued via `model_fetcher` → picker p) 
* diff review before applying (`D`) — only works on the path you want.

**Future priorities:** JSON presets, keybinding rebinding (toml), Windows service tray/config for packaged releases (?), release tags; error log with colored severity (P2), hot-load project on `Ctrl+R`.
