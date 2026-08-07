# Actionable TODO List (TODO.md)
## Ocoger (OpenCode Manager)

---

## Priority Legend
* **P0:** Critical Path / Blocker (MVP Required)
* **P1:** High Priority (Core Feature)
* **P2:** Medium Priority (Enhancement)
* **P3:** Low Priority / Nice-to-Have

---

## 1. Project Setup & Core Foundation
- [x] **[P0]** Initialize Cargo binary project (`cargo new ocoger`). *(used `cargo init` in-repo; edition 2021, MSRV 1.75)*
- [x] **[P0]** Configure `Cargo.toml` with dependencies (`ratatui`, `crossterm`, `tokio`, `gray_matter`, `reqwest`, `serde`, `clap` + `serde_yaml`, `anyhow`, `thiserror`). **Deviation:** `comment-json` is a JS-only crate; substituted `serde_json5` per ARCH fallback — see open spike below.
- [x] **[P0]** Setup logging infrastructure using `tracing-subscriber` + `tracing-appender` writing to `.ocoger.log`.
- [x] **[P0] Spike:** JSONC comment preservation. **Verdict: `jsonc-parser = { version = "0.33", features = ["cst","serde"] }`** — CST `object_value_or_set() -> get(key) -> prop.set_value(...)` performs surgical single-key edits while preserving all comments/trailing commas/formatting byte-for-byte (validated by `examples/jsonc_spike.rs`, 15/15 checks incl. byte-delta == value-diff). Same technique as TS `sst/opencode` (Microsoft jsonc-parser modify/applyEdits). `serde_json5` rejected (lossy).

## 2. Core Data Engines
- [x] **[P0]** Implement `AgentFile` struct and `.opencode/agents/` file scanner in `src/core/agent_scanner.rs`. *(non-recursive for MVP; recursion P2)*
- [x] **[P0]** Build frontmatter parser wrapper around `gray_matter` in `src/core/agent_parser.rs`. *Design: split-at-delimiter fidelity — returns original raw YAML slice verbatim; avoids reserialization key-order/comment drift. `ParsedAgent` holds raw yaml + body.*
- [x] **[P0]** Unit test: Verify parsing and re-writing YAML frontmatter preserves exact Markdown body text. *Strengthened: fixture asserts byte-identical body **and** raw YAML (comments, key order) — catches beta fidelity risk beyond PRD's body-only contract.*
- [ ] **[P1]** Implement JSONC config reader and writer for `opencode.jsonc` in `src/core/jsonc_config.rs`.
- [ ] **[P1]** Unit test: Ensure comments and trailing commas in `opencode.jsonc` remain intact after mutation.

## 3. Terminal User Interface (Ratatui)
- [ ] **[P0]** Build main application layout (Header, Agent List Pane, Parameter Form Pane, Log Bar).
- [ ] **[P0]** Implement keyboard event loop in `src/ui/event_handler.rs` handling `j`/`k`, `Space`, `Tab`, `q`.
- [ ] **[P0]** Implement agent multi-selection toggle logic.
- [ ] **[P1]** Implement dynamic Model Picker Popup widget with real-time text input filtering.
- [ ] **[P1]** Add visual indicator badge for modified unsaved agents (`[*]`).

## 4. Async Provider Model Fetcher
- [ ] **[P1]** Implement async worker function `fetch_v1_models(base_url, api_key)` using `reqwest`.
- [ ] **[P1]** Build model catalog cache layer storing fetched model strings.
- [ ] **[P2]** Implement static fallback list for Anthropic native endpoints.
- [ ] **[P2]** Add connection timeout and user-friendly error display in TUI log panel for offline endpoints.

## 5. Process Supervision & Automation
- [ ] **[P1]** Implement `ProcessManager` struct using `tokio::process`.
- [ ] **[P1]** Build cross-platform signal sender for graceful process shutdown (`SIGTERM`/Windows kill).
- [ ] **[P1]** Bind `Ctrl+S` hotkey to run atomic file save + process restart sequence.
- [ ] **[P2]** Pipe subprocess `stdout` and `stderr` streams into `tui` bottom drawer widget.

## 6. Testing, CI & Packaging
- [ ] **[P1]** Integration test: Test bulk model change across 20 synthetic agent files.
- [ ] **[P2]** Setup GitHub Actions workflow for automated `cargo test` and cross-compilation builds.
- [ ] **[P3]** Write shell completion scripts (`bash`, `zsh`, `fish`).