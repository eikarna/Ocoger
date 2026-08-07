# Business Requirements Document (BRD)
## Ocoger (OpenCode Manager)

---

| Metadata | Value |
| :--- | :--- |
| **Project Title** | Ocoger (OpenCode Manager) |
| **Document Version** | 1.0.0 |
| **Status** | Approved / Architecture Phase |
| **Target Runtime** | Rust (Native Terminal UI) |
| **Target Platform** | Cross-platform (Linux, macOS, Windows) |

---

## 1. Executive Summary

OpenCode is an open-source AI coding assistant framework that delegates specialized development tasks across subagents defined via Markdown frontmatter (`.opencode/agents/*.md`) and a global provider configuration file (`opencode.json` / `opencode.jsonc`).

While highly extensible, developer productivity is currently hindered by significant configuration friction:
1. **Lack of Hot-Reloading:** Any modification to agent configurations or global settings requires manually stopping and restarting the OpenCode process.
2. **Configuration Fragmentation:** Model selections, parameters (e.g., `temperature`, `top_k`, `top_p`), and system prompts are scattered across dozens of agent files. Updating model assignments across agents requires tedious, repetitive manual file editing.
3. **Provider Endpoint Disconnect:** Users must manually lookup and type complex model identifier strings instead of picking available models directly from provider endpoints (`/v1/models`).

The **Ocoger (OpenCode Manager)** is a lightweight, high-performance terminal user interface built in Rust. It unifies global settings and subagent management, auto-discovers active provider models via REST APIs, enables bulk configuration updates across agents, and supervises the OpenCode execution process with automated process restarts.

---

## 2. Business Objectives & Key Performance Indicators (KPIs)

### 2.1 Strategic Objectives
* **Drastically Reduce Developer Friction:** Streamline context-switching and manual text editing down to single keypress actions within a central TUI dashboard.
* **Eliminate Configuration Errors:** Eliminate typos in model strings and invalid JSON parameter values via validation and live API model lookup.
* **Accelerate Agent Iteration Cycles:** Provide a seamless feedback loop where configuration changes trigger near-instant process updates.

### 2.2 Key Performance Indicators (KPIs)
* **Configuration Time Reduction:** Reduce time to update model settings across 10+ subagents from ~5 minutes to under 5 seconds (>98% efficiency gain).
* **Zero Configuration Corruption:** 100% preservation of non-standard JSONC comments and custom agent body text during read/write cycles.
* **Sub-50ms Response Time:** Instant TUI render and keyboard navigation latency utilizing Rust's native asynchronous concurrency (`tokio`) and immediate-mode UI (`ratatui`).

---

## 3. Scope & Feature Boundaries

### 3.1 In-Scope
* **Subagent Frontmatter Management:** Batch and single-agent editing of frontmatter parameters (`model`, `temperature`, `top_k`, `top_p`, `reasoning_effort`).
* **Global Provider Config Management:** Parsing and editing `opencode.json` / `opencode.jsonc` files, specifically supporting `custom_headers`, `extra_body`, default models, and provider endpoints.
* **Dynamic Model Auto-Discovery:** Querying `/v1/models` across configured OpenAI-compatible endpoints with fallback options for static model lists (e.g., Anthropic native endpoints).
* **Process Supervision:** Spawning, monitoring, signaling (`SIGTERM`/`SIGKILL`), and auto-restarting the OpenCode core process upon configuration saves.
* **Preservation of Comments & Structure:** Preserving YAML structure and JSONC comments without destroying user annotations.

### 3.2 Out-of-Scope (Version 1.0)
* Graphical User Interface (GUI) wrapper (Focus remains exclusively on Terminal UI).
* Direct manipulation of LLM backend API keys within cloud storage (All keys remain in local config).
* Remote SSH agent configuration syncing (Local filesystem operations only for v1.0).

---

## 4. Stakeholders & Target Audience

* **Primary Users:** Software engineers, AI researchers, and open-source contributors using OpenCode as their primary coding assistant.
* **Secondary Users:** System integrators and prompt engineers maintaining custom subagent workflows and multi-provider LLM setups.

---

## 5. Non-Functional Requirements

| ID | Category | Requirement Description |
| :--- | :--- | :--- |
| **NFR-01** | Performance | Memory footprint must remain under 30 MB during full operation. |
| **NFR-02** | Latency | Keypress to UI state update must take < 16ms (60 FPS target). |
| **NFR-03** | Safety | File writes must be atomic (write to temporary file, then rename) to prevent config corruption during crashes. |
| **NFR-04** | Compatibility | Full cross-platform support for Windows Terminal, iTerm2, Alacritty, Kitty, and WezTerm. |
| **NFR-05** | Reliability | TUI crash must not disrupt or corrupt the running `opencode` daemon process. |