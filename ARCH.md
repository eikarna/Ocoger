# System Architecture & Design Document (ARCH.md)
## Ocoger (OpenCode Manager)

---

## 1. System Architecture Overview

`ocoger` follows the **Model-View-Update (MVU)** pattern over `ratatui` + `crossterm`, async via `tokio`.

```text
                         +-----------------------------------+
                         |             UI (TUI)              |
                         | ratatui + crossterm event loop    |
                         +-----------------+-----------------+
                                           |
                    +----------------------+----------------------+
                    |                                             |
    +---------------+---------------+          +----------------+--------------+
    |           App (MVU)          |          |        State & Keymaps       |
    | ui/app.rs (pure model)       | --------> | Action / Mode (pure logic)  |----> ui/widgets/*
    | state: agents, dirty, log,  |          | stack per TODO.md            |
    | staged model, staged pkeys  |          | render only observable state |
    | App.update(...)             |          | (feeds ratatui render)      |
    +---------------+---------------+          +----------------+--------------+
                    |                                             |
                    v                                             v
         +----------+-----------+                    +------------+------------+
         | Core Services         |                    |     Services / DI      |
         | --------------------- |                    | ---------------------- |
         | agent_parser (md+yaml)|                    | process_manager        |
         | agent_scanner         |                    |  (tokio::process)      |
         | diff                  |                    | model_fetcher          |
         | fs_util               |                    | jsonc_config (CST)    |
         +--------+--------------+                    | (background tasks)     |
                  |                                   +-----------+------------+
                  |                                               |
                  v                                               v
          +  file system state   |                  external opencode process,
          | .opencode/agents/  +------------------+ opencode.json(c), /v1/models |
          | staging mirrors    |        I/O        +-----------------------------+
          +--------------------+                  (services only, no TUI code)
```

## 2. Core Modules Specification

### 2.1 Agent frontmatter (`src/core/agent_parser.rs`)
* Same responsibility: parse/mutate/write `.opencode/agents/*.md`.
* Data model: `AgentFile` holds:
  * `frontmatter: AgentFrontmatter` (typed)
  * `raw_yaml` / `raw_body` strings (bytes preserved verbatim)
  * `is_selected` (ui), `is_dirty` (data-loss safeguard)
* CSP model: `load_agent(...) -> ParsedAgent { raw_yaml, raw_body }` then merge into `AgentFile`.
* Writes: append-only regex splice on `raw_yaml` (`update_models`), then atomic write to disk via `agent_diffs`.
* `is_dirty` persists through `AgentFile::save_and_check_restart` to drive `/v1/models` and the diff view.

### 2.2 JSONC engine (`src/core/jsonc_config.rs`)
* Uses `jsonc-parser` CST edits — **byte-surgical** (validated 15/15 in `examples/jsonc_spike.rs`).
* Public API is read/write via CST:
  * `JsoncConfig::load` → raw → `parse_cst → CstRootNode`
  * `config_items()` → first-level fields + provider options flatten per TODO §2.2.
  * `set_model` / `set_nested_str(path, value)` (parent-walking CST) — preserves comments every time.
  * `save()` → atomic write → ` opencode.jsonc ` (idempotent).
* Rules: never serialize via serde on write; `parse_to_serde_value` used only for runtime type checking.

### 2.3 Process supervisor (`src/services/process_manager.rs`)
* Lifecycle: `spawn` → stdout/stderr pipes → appends to channel via tasks → each frame drains the queue.
* API: `kill()` → wait ≤ 3s → `restart()`. Windows-aware: process kill uses the
  platform implementation, plus explicit `tokio::process::kill_on_drop` because Android/Windows
  don't naturally SIGTERM the child.
* Adds `find_executable`: PATH lookup for `opencode` with Windows shim support (`.cmd`, `.exe`, `.bat`).
* Coordination: when an agent file is saved in the UI, the App notifies the manager via restart so
  hot-restart (Ctrl+S, PRD §4) is atomic. File writes are awaited before restart to avoid "saved
  agent then process failed" panics.

### 2.4 Model fetcher (`src/services/model_fetcher.rs`)
* Hand-rolled `/v1/models` client with `reqwest` + configurable env (`api_key_env` from JSONC if present).
* Static fallback: `ANTHROPIC_NATIVE_MODELS` (immutable list, mixed in with fetched results).
* Fetch triggers on `/v1/models` fetch via `fetch_v1_models` with a hard 10s timeout; errors are
  reported per-endpoint into App log.
* Fetch results are buffered in `App.shared_catalog` (HashSet), merged into picker_catalog via
  `sync_catalog_from_shared` which is now pure change-driven (populates only when something batches).

### 2.5 Diff engine (`src/core/diff.rs`)
* Provides `unified_by_line` (similar TextDiff::iter_all_changes) and `agent_diffs`: per-agent diff rows of
  the form `file_name: old =
  (disk) → staged` with +/- lines; writes "staged" mirrors to `.ocoger/staging/` via atomic write and
  renames. The mirror stages edits while the user reviews; atomically committed only via `s`.

## 3. Technology Stack & Crate Selection

| Dependency | Purpose | Justification |
|---|---|---|
| `ratatui` | TUI | De facto standard immediate-mode TUI. |
| `crossterm` | Terminal backend | Multi-platform, event stream. |
| `tokio` | Async runtime | Process I/O + concurrent provider fetch. |
| `gray_matter` | Frontmatter | Minimal AST-less YAML split. |
| `jsonc-parser` | JSONC CST | Byte-surgical edits, comment preservation validated. |
| `reqwest` | HTTP (models) | native Rust; async connection pooling; rustls. |
| `similar` | Diff | Unified diff text rendering; `TextDiff::iter_all_changes` |
| `tracing` / `tracing-subscriber` | Logging | Structured internal logging. |

## 4. Error Handling & Data Integrity (revised)

1. **Atomic File Persistence:** still via `core::fs_util::atomic_write` (tmp sibling → `fs::rename`). Now applied to user saves as well as the `.ocoger/staging/` mirrors.
2. **Formatting/Encoding Byte-Fidelity:** verified via tests on parse→serialize round-trips covering:
   * raw YAML slices with comments properly anchored (per `quoted_value_replacement_keeps_anchor_comment`)
   * presence of ` # inline divider` and a ```` code fence with `---`
   * Key ordering preserved on partial updates (`triple comments`)
   * Unicode/typographic fidelity (non-ASCII filenames safe for 'PathBuf').
3. **Process Supervision Under Failure:** ProcessManager handles kill timeouts, zombie processes, and a `/v1/models` failure by idempotent restart; errors logged distinctly in App log; state machine survives blocked render loop.
4. **Config write back-pressure:** `config_items` extraction is immutable-then-sync: config file changes are staged until save; any JSONC write triggers a `sync_catalog_from_shared` on the next tick to keep the picker in sync.
5. **TUI render loop protection:** App state updates must never block the render loop (NFR <16ms). `sync` uses `try_read`-try-write guards under `SharedCatalog` (tokio lock; see `delayed_keymaps` dodges for jitter when models are being fetched).

---

## 5. Keymap contract (Phase 3/4 state)

| Key | Behavior |
|---|---|
| `j/k` / `Down/Up` | Move cursor (band-local in Form, global list otherwise) |
| `Space` | Toggle selection |
| `a` | Toggle select-all/deselect-all |
| `m` | Model edit modal (list → ModelEdit) |
| `e`/`g` | Enter form; (Tab toggles band per TODO stack) |
| `d` | Diff preview (any mode; only when dirty or per-mode current selection) |
| `s`/`Ctrl+S` | Save-trigger; only if dirty, triggers ProcessManager restart |
| `r` | Manual reagent (later phase) |
| `q`/`Esc` | Quit/contextual back (within modals) |

## 6. Risk Areas & Open Questions (for next maintainer)

1. **JSONC vs model-edit flow:** `set_model`/`ModelEdit` modal persists until `s`; quick edits made in browser are not emitted till the next tick. Zero-memory intention: the real table is a View (no cache) — acquires `Picker` on Enter and closes? Mapping the exact start/final lifecycle depends on the final cost-tracking update cadence.
2. **Multiple base_urls per provider:** currently one shared url per provider label; reevaluate whether opencode wants a list for multi-host (QuickStart allows one only).
3. **Windows service operation:** process manager should likely use a service API in addition to `tokio::process` spawn (more robust to `Get-CmdLine` and recycling); spike approval pending.
4. **Packaging:** see ROADMAP Phase 4; a `tui` release binary for Linux/macOS/Windows (Windows `rustup target x86_64-pc-windows-msvc` etc.)
5. **Hot-reload consistency:** diff preview should **probably be cached per agent file** to avoid re-reading the `.ocoger/staging/*` on every rendered frame (currently recomputed on every open when mismatches are detected).
6. **Anti-regression:** the number of tests in the tree grew to 28; more integration tests live at `core::fs_util::tests::create_and_rename_tmp_file` + `ui::app::tests::save_writes_selected_files_and_clears_dirty` (full cycle) — commit already contains these.

## 7. Phase 5 — Hub-and-Spoke Architecture

The current shell is a single-surface app (Subagents list). Phase 5 promotes it to a hub-and-spoke TUI (lazygit-style): a new `Mode::MainMenu` boots by default and dispatches into pane-local `Mode`s. Existing `Mode`s remain unchanged; they are reused as leaf panes.

```text
         ┌──────────────────────────┐
         │  Mode::MainMenu (boot)   │  ← new; list + Enter/Esc
         └────────────┬─────────────┘
        ┌─────────────┼─────────────┬──────────────┬──────────────┬─────────────┐
        ▼             ▼             ▼              ▼              ▼             ▼
   Subagents    Providers &    Permissions    MCP Servers    Commands      Process &
   (current     Models         (5.4)          (5.3)          (5.2)         Logs (5.6)
    List mode)  (5.5)
```

Design rules (kept invariant across panes):

* One `Mode` enum; `handle_key` dispatches via `app.mode`. New pane = new variant + widget under `src/ui/widgets/<pane>.rs`.
* `ModalFocus` Input↔List (Tab-toggle) applies wherever a modal has *both* a filter box and a bound action set. Panes with only one side do not need it.
* CST-surgical config edits go through `JsoncConfig::{set_nested_str,set_value,append}` — never serde round-trip. New panes add dedicated helpers in `core/jsonc_config.rs` rather than ad-hoc string splicing.
* New keybinds live in `core::keymap::defaults()` under the pane's `Mode` and are user-overridable via `keymaps.toml` (same precedence chain).
* Process supervision is global state owned by `App` (via `ProcessManager`), surfaced by footer in every pane and by the dedicated Process & Logs pane.
* Multi-pane dirty tracking stays single-pass: `is_dirty` counts / `dirty_count()` union across modules as they appear — restart semantics continue to trigger from any dirty write.

## 8. Reference: Tail log section example

```text
 ocoger :: 3 agent(s) | 2 selected | 0 unsaved | mode: List | running...
```
Footer log pane shows:
* `[stdout] Attempting to connect to Ollama at :11434`
* `[stdout] model-discovery response: added 3 models from anthropic, 8 from openrouter`
* `[stderr] WARN: opencode exited non-zero`
* `restarting process (pid=39348)`

User actions log:
* `[jsonc] Saved 2 agent(s)` (on `s`)
* `[ditf] model 'deepseek-r1' fusion on 2 agents`

Process state is echoed immediately into the header of every render tick.

---

*This document reflects the code as validated by `cargo test/clippy/fmt` (28 tests, zero warnings) and spike run in `examples/proc_spike.rs` for Windows process semantics. See ROADMAP.md for Phase status.*