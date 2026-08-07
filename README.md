# Ocoger (OpenCode Manager)

> A blazingly fast, native Rust TUI for managing OpenCode subagent configurations, global JSONC settings, dynamic model discovery, and process restarts.

---

## Key Features

* 🚀 **Zero Friction Configuration:** Edit single or multiple subagent configurations (`.opencode/agents/*.md`) simultaneously without manually opening text editors.
* 🤖 **Dynamic Model Auto-Discovery:** Query `/v1/models` directly across your configured LLM providers (OpenRouter, Ollama, LM Studio, vLLM) with live fuzzy search filtering.
* 📝 **JSONC Comment Preservation:** Parse and update `opencode.json` / `opencode.jsonc` (`custom_headers`, `extra_body`, base URLs) while preserving your inline comments and formatting.
* 🔄 **Hot-Restart Automation:** Automatically terminates and restarts your active `opencode` instance upon saving configuration changes (`Ctrl+S`).
* ⚙️ **Parameter Fine-Tuning:** Easily adjust parameters like `temperature`, `top_k`, `top_p`, and `reasoning_effort` via intuitive TUI controls.
* ⚡ **Native Rust Performance:** Extremely lightweight (<30MB RAM footprint) built on `ratatui` and `tokio`.

---

## Installation

### Prerequisites
* Rust toolchain (MSRV 1.75.0+)
* An active OpenCode installation

### Building from Source
```bash
# Clone repository
git clone [https://github.com/your-username/ocoger.git](https://github.com/your-username/ocoger.git)
cd ocoger

# Build release binary
cargo build --release

# Optional: Install to system PATH
cargo install --path .

```

---

## Keybindings Quick Reference

| Hotkey | Action Description |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Navigate through subagent list |
| `Space` | Toggle selection checkmark on active agent |
| `a` | Select / Deselect all agents |
| `m` | Open interactive Model Selector Modal (fetches `/v1/models`) |
| `e` | Switch focus to Agent Parameter Form |
| `g` | Switch focus to Global `opencode.jsonc` Settings |
| `s` or `Ctrl+S` | **Atomic Save All Configurations & Restart OpenCode Process** |
| `r` | Force manual restart of OpenCode daemon |
| `Tab` | Cycle focus between UI panels |
| `q` or `Esc` | Exit Application |

---

## Configuration Architecture

`ocoger` operates at the intersection of two OpenCode configuration boundaries:

1. **Subagent Metadata (`.opencode/agents/*.md`):**
```yaml
---
model: deepseek-r1
temperature: 0.7
top_p: 0.9
reasoning_effort: high
---
You are an expert Rust backend engineer...

```


2. **Global Provider Settings (`opencode.jsonc`):**
```jsonc
{
  // Primary LLM Provider Configuration
  "provider": "openrouter",
  "base_url": "[https://openrouter.ai/api/v1](https://openrouter.ai/api/v1)",
  "custom_headers": {
    "X-Title": "My Local OpenCode Studio"
  },
  "extra_body": {
    "transforms": ["middle-out"]
  }
}

```



---

## Project Layout

```text
ocoger/
├── Cargo.toml
├── BRD.md
├── PRD.md
├── ARCH.md
├── ROADMAP.md
├── TODO.md
├── README.md
└── src/
    ├── main.rs
    ├── core/
    │   ├── agent_parser.rs
    │   ├── agent_scanner.rs
    │   └── jsonc_config.rs
    ├── services/
    │   ├── model_fetcher.rs
    │   └── process_manager.rs
    └── ui/
        ├── app.rs
        ├── event_handler.rs
        └── widgets/
            ├── agent_list.rs
            ├── model_picker.rs
            └── log_drawer.rs

```

---

## License

Distributed under the MIT License. See `LICENSE` for details.