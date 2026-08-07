# System Architecture & Design Document (ARCH.md)
## Ocoger (OpenCode Manager)

---

## 1. System Architecture Overview

`ocoger` follows a modular, decoupled Architecture built around the **Model-View-Update (MVU)** design pattern, optimized for immediate-mode terminal interfaces using `ratatui`.

```text
                       +-------------------------------+
                       |        CLI & Main Loop        |
                       |      (crossterm / tokio)      |
                       +---------------+---------------+
                                       |
                                       v
                       +---------------+---------------+
                       |        App State Manager      |
                       +-------+---------------+-------+
                               |               |
         +---------------------+               +---------------------+
         |                                                           |
         v                                                           v
+--------+----------------------+                   +----------------+---------------+
|     UI Rendering Layer        |                   |      Core Service Layer        |
|  - Agent ListView Widget     |                   |  - Frontmatter Parser Engine  |
|  - Form/Slider Widgets        |                   |  - JSONC Parser Engine        |
|  - Model Picker Modal         |                   |  - Provider Async Client      |
|  - Process Log Terminal       |                   |  - Process Supervisor Daemon  |
+-------------------------------+                   +----------------+---------------+
                                                                     |
                                                                     v
                                                    +----------------+---------------+
                                                    |    Disk I/O & External APIs    |
                                                    |  - .opencode/agents/*.md       |
                                                    |  - opencode.json(c)            |
                                                    |  - HTTP GET /v1/models         |
                                                    |  - Subprocess PID & Signals    |
                                                    +--------------------------------+

```

---

## 2. Core Modules Specification

### 2.1 Frontmatter Parser (`src/core/agent_parser.rs`)

* **Responsibility:** Parse, mutate, and write back YAML frontmatter in Markdown files.
* **Crate Dependencies:** `gray_matter`, `serde`, `serde_yaml`.
* **Data Structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

pub struct AgentFile {
    pub path: PathBuf,
    pub frontmatter: AgentFrontmatter,
    pub raw_body: String,
    pub is_selected: bool,
}

```



### 2.2 JSONC Engine (`src/core/jsonc_config.rs`)

* **Responsibility:** Read, edit, and serialize `opencode.json` / `opencode.jsonc` preserving user comments.
* **Crate Dependencies:** `comment-json` or `serde_json5`.
* **Data Structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub custom_headers: Option<std::collections::HashMap<String, String>>,
    pub extra_body: Option<serde_json::Value>,
}

```



### 2.3 Provider API Discovery (`src/services/model_fetcher.rs`)

* **Responsibility:** Non-blocking async execution fetching available models from registered endpoints.
* **Crate Dependencies:** `reqwest`, `tokio`.
* **Execution Flow:**
1. Parse provider base URLs from `opencode.jsonc`.
2. Spawn async worker task using `tokio::spawn`.
3. Perform `GET /v1/models` with Bearer authentication.
4. Deduplicate model IDs and publish results to shared `Arc<RwLock<ModelCatalog>>`.



### 2.4 Process Supervisor (`src/services/process_manager.rs`)

* **Responsibility:** Lifecycle management of the `opencode` execution binary.
* **Crate Dependencies:** `tokio::process`, `nix` / `windows-sys`.
* **Restart Sequence:**
1. Trigger event received (e.g., `SaveAndRestart`).
2. Send graceful termination signal (`SIGTERM`) to active child process PID.
3. Wait with timeout (3000ms). If process fails to exit, send `SIGKILL`.
4. Flush logs to log pane.
5. Re-spawn `opencode` child process and capture `stdout`/`stderr` streams.



---

## 3. Technology Stack & Crate Selection

| Dependency | Purpose | Justification |
| --- | --- | --- |
| `ratatui` | Terminal UI Layout & Widgets | De facto standard high-performance TUI framework in Rust. |
| `crossterm` | Terminal Backend & Event Handling | Reliable cross-platform terminal control (ANSI/Windows Console). |
| `tokio` | Async Runtime & Process Control | Required for background HTTP polling and child process streaming. |
| `gray_matter` | YAML Frontmatter Extraction | Isolates frontmatter from Markdown body cleanly without full AST parse. |
| `comment-json` | JSONC Parsing | Preserves formatting and `//` comments in `opencode.jsonc`. |
| `reqwest` | HTTP Client | Features `json` support, async connection pooling, and rustls. |
| `clap` | Command Line Argument Parsing | Clean CLI argument handling (e.g., specifying custom config paths). |
| `tracing` + `tracing-appender` | Logging Architecture | Non-blocking structured internal application logging. |

---

## 4. Error Handling & Data Integrity

1. **Atomic File Persistence:**
* Files are written to a temporary sibling file (`.file.md.tmp`) first.
* `fs::rename` is invoked to overwrite the original target atomically, guaranteeing zero partial writes on power loss or crash.


2. **Parsing Fallback:**
* If an agent file has malformed YAML, the UI marks the file with an `[Error]` flag and prevents overwrite until rectified by the user, avoiding accidental data destruction.