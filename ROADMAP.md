# Product Roadmap (ROADMAP.md)
## Ocoger (OpenCode Manager)

---

## Roadmap Timeline Overview

```text
[ Phase 1: MVP Core ] --------> [ Phase 2: Provider Integration ] --------> [ Phase 3: Process Supervisor ] --------> [ Phase 4: Enterprise & Presets ]
(Weeks 1-2)                      (Weeks 3-4)                             (Weeks 5-6)                              (Weeks 7-8)

```

---

## Detailed Milestones

### Phase 1: MVP & Local Configuration Engine (Target: Week 2)

* [ ] **Project Scaffolding:** Cargo workspace initialization, module architecture setup.
* [ ] **Subagent Scanner:** Implement `walkdir` scan for `.opencode/agents/*.md`.
* [ ] **Frontmatter Parser:** Integrate `gray_matter` to extract and modify agent YAML metadata.
* [ ] **Basic TUI Interface:** Build primary list view with multi-select capability (`Space`) using `ratatui`.
* [ ] **Single & Batch Model Editor:** Enable manual text entry for changing model keys across tagged agents.
* [ ] **Atomic File Saver:** Implement safe disk persistence pipeline.

### Phase 2: Provider API Discovery & JSONC Engine (Target: Week 4)

* [ ] **JSONC Parser Integration:** Implement `comment-json` parser for `opencode.json` / `opencode.jsonc`.
* [ ] **Global Config UI:** Add tab view to inspect and edit `custom_headers` and `extra_body`.
* [ ] **Async Model Discovery Engine:** Build background worker using `reqwest` to query `/v1/models` across configured providers.
* [ ] **Interactive Model Picker Modal:** Build searchable modal with live filtering for model selection.
* [ ] **Static Fallback Catalog:** Embed static lookup lists for Anthropic and non-standard endpoints.

### Phase 3: Process Supervision & Live Logging (Target: Week 6)

* [ ] **Process Manager Engine:** Implement `tokio::process::Command` wrapper for spawning `opencode`.
* [ ] **Signal Controller:** Cross-platform `SIGTERM` / `SIGKILL` termination pipeline.
* [ ] **Hot-Restart Automation:** Hook `Ctrl+S` (Save Config) to trigger automated process termination and restart.
* [ ] **Live Terminal Output Drawer:** Embedded stdout/stderr stream consumer and log buffer widget.

### Phase 4: Presets, Profiles & Polish (Target: Week 8)

* [ ] **Configuration Presets:** Save/Load profile snapshots (e.g., "Deep Work / Reasoning Mode", "Fast Draft / Cheap Mode").
* [ ] **Diff Previewer Modal:** Visual side-by-side diff display of pending configuration modifications before saving.
* [ ] **Custom Hotkey Rebinding:** Configurable keybindings file (`tui_keymaps.toml`).
* [ ] **Binary Packaging:** Release pipeline for GitHub Actions producing prebuilt binaries for Linux, macOS, and Windows.
