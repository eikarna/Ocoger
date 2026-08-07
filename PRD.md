# Product Requirements Document (PRD)
## Ocoger (OpenCode Manager)

---

| Document Detail | Value |
| :--- | :--- |
| **Product Name** | Ocoger (OpenCode Manager) |
| **Binary Name** | `ocoger` |
| **Version** | 1.0.0-draft |
| **Author** | Lead Systems Architect |

---

## 1. Product Overview

`ocoger` is a high-performance terminal dashboard built in Rust using `ratatui` and `crossterm`. It solves the operational friction of managing OpenCode agent configurations (`.opencode/agents/*.md`) and global provider settings (`opencode.json` / `opencode.jsonc`).

---

## 2. User Personas & Use Cases

### Personas
1. **Power Developer ("Alex"):** Maintains 15+ subagents for coding, linting, architecture, and docs. Frequently swaps models depending on cost, rate limits, and task complexity (e.g., switching from `claude-3-5-sonnet` to `deepseek-r1` for reasoning tasks).
2. **Local AI Tinkerer ("Budi"):** Uses Ollama/LM Studio alongside cloud models. Needs fast `/v1/models` auto-discovery to pick locally loaded GGUF models.

---

## 3. Detailed Feature Specifications

### Epic 1: Subagent Frontmatter & Markdown Management
* **FE-1.1 File Discovery:** Auto-scan `.opencode/agents/` directory recursively for `.md` files.
* **FE-1.2 Frontmatter Parsing:** Parse YAML frontmatter metadata (`model`, `temperature`, `top_k`, `top_p`, `reasoning_effort`) while leaving the Markdown body untouched.
* **FE-1.3 Bulk Model Swapper:**
  * Multi-select agents in a list view using `Space`.
  * Press `M` to bring up the Model Selector Modal.
  * Apply selected model across all tagged agents simultaneously.
* **FE-1.4 Parameter Tuning:** Dedicated slider/input fields for numeric parameters (`temperature` 0.0–2.0, `top_k`, `top_p`).

### Epic 2: Global Configuration (`opencode.jsonc`) Management
* **FE-2.1 Comment-Aware Parsing:** Read and parse `opencode.json` or `opencode.jsonc` while preserving inline comments (`//`) and trailing commas using `comment-json` / `json5`.
* **FE-2.2 Header & Body Customization:** Panel to edit `custom_headers` (e.g., `HTTP-Referer`, custom auth headers) and `extra_body` parameters (e.g., `reasoning_effort`, `frequency_penalty`).
* **FE-2.3 Provider Endpoint Management:** Manage API base URLs, default provider keys, and global default models.

### Epic 3: Dynamic Model Auto-Discovery (`/v1/models`)
* **FE-3.1 Async Polling:** Background task using `reqwest` querying `<provider_base_url>/v1/models` with authorization headers.
* **FE-3.2 Fallback Catalog:** Built-in static catalog for non-standard APIs (e.g., Anthropic native, custom endpoints) when `/v1/models` is unavailable.
* **FE-3.3 Live Filtering:** Interactive fuzzy search filter over fetched model IDs.

### Epic 4: Process Supervision & Hot-Restart Simulation
* **FE-4.1 Process Launcher:** Ability to spawn `opencode` process directly as a managed subprocess using `tokio::process`.
* **FE-4.2 Auto-Restart Trigger:** Automatically send `SIGTERM` (or process termination signal on Windows), wait for clean exit, and re-spawn `opencode` immediately when config changes are saved (`Ctrl+S`).
* **FE-4.3 Log View:** Embedded bottom-drawer terminal viewer displaying real-time stdout/stderr logs from the `opencode` subprocess.

---

## 4. UI/UX & Keyboard Control Specs

```text
+-----------------------------------------------------------------------------------+
| OCOGER v1.0.0                                    [Status: OPENCODE RUNNING (12043)]|
+-----------------------------------+-----------------------------------------------+
| SUBAGENTS (.opencode/agents/*.md) | CONFIGURATION EDITOR                          |
|                                   | Target: agent-coder.md                        |
| [x] 01. agent-coder.md            | --------------------------------------------- |
| [ ] 02. agent-reviewer.md         | Model           : [ deepseek-r1          ] (v) |
| [x] 03. agent-architect.md        | Temperature     : [====|-----] 0.70           |
| [ ] 04. agent-docs.md             | Top P           : [========|-] 0.90           |
|                                   | Reasoning Effort: [ high                 ]    |
|                                   | --------------------------------------------- |
|                                   | GLOBAL PROVIDER SETTINGS (opencode.jsonc)     |
|                                   | Base URL        : [https://openrouter.ai/api/v1](https://openrouter.ai/api/v1)|
|                                   | Custom Headers  : {"X-Title": "MyProject"}    |
+-----------------------------------+-----------------------------------------------+
| LOGS & PROCESS OUTPUT                                                             |
| Info: Applied model 'deepseek-r1' to 2 subagents.                      |
| Info: Triggered OpenCode restart (PID 12043 terminated -> PID 12099)   |
+-----------------------------------------------------------------------------------+
| [Space] Tag  [a] Select All  [m] Change Model  [s] Save & Restart  [q] Quit       |
+-----------------------------------------------------------------------------------+
```

### Keyboard Shortcuts

| Key | Action |
| --- | --- |
| `j` / `k` or `Down` / `Up` | Navigate agent list |
| `Space` | Toggle selection tag on agent |
| `a` | Toggle select/deselect all agents |
| `m` | Open dynamic Model Picker Modal |
| `e` | Focus Parameter/Frontmatter Edit Panel |
| `g` | Focus Global Config (`opencode.jsonc`) Panel |
| `s` or `Ctrl+S` | Atomic Save All + Trigger OpenCode Restart |
| `r` | Manual Force Restart OpenCode Process |
| `Tab` | Switch between active UI panes |
| `q` or `Esc` | Quit Application |
---

## 5. System State Machine

```
              +----------------------+
              |     INITIALIZING     |
              +----------+-----------+
                         |
                         v
              +----------------------+
              |    SCAN & PARSE      |
              | (Agents + JSONC)     |
              +----------+-----------+
                         |
                         v
              +----------------------+
  +----------->   IDLE / DASHBOARD   <-----------+
  |           +----+-----------+-----+           |
  |                |           |                 |
  | (Modal Close)  |           | (Press 'm')     | (Press 's')
  |                v           v                 |
  |           +----+----+  +---+-------------+   |
  |           | BATCH   |  | FETCH /v1/models|   |
  |           | EDIT    |  | MODEL PICKER    |   |
  |           +----+----+  +---+-------------+   |
  |                |           |                 |
  |                +-----+-----+                 |
  |                      |                       |
  |                      v                       |
  |             +----------------+               |
  +-------------+ ATOMIC SAVE &  +---------------+
                | PROCESS RESTART|
                +----------------+
```