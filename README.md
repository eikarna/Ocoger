# Ocoger (OpenCode Manager)

> A blazingly fast, native Rust TUI for managing OpenCode subagent configurations, global JSONC settings, dynamic model discovery, and process restarts.

---

## Status as of 2026-08-08 — Phase 1–3 Complete (All MVP "Must-Have" Features Shipped)

* ✅ Subagent frontmatter management with **top-level multi-select**, model field editing, and atomic save (`s`/`Ctrl+S`).
* ✅ Comment-preserving JSONC config management (via `jsonc-parser` CST — substitution validated 15/15 in the spike).
* ✅ Model auto-discovery: `ANTHROPIC_NATIVE_MODELS` fallback + `/v1/models` fetch with `mpsc` log reports, live filter, and merge-on-change dedupe.
* ✅ Process supervision: `tokio::process` kill/restart with 3s `wait_timeout` and explicit Windows-safe lookup (`opencode` `.cmd/.exe/.bat`).
* ✅ Diff preview: pre-save unified diff against staged `.ocoger/staging/` mirrors (before commit) for any changed agent.
* ✅ Full hot-restart with restart emission **only after disk writes land**.

Currently in **Phase 4** (polish): presets, profiles, keybinding rebinding, packaging.

---

## Key Features

* 🚀 **Zero-Friction Configuration:** edit single or multiple subagent configs (`.opencode/agents/*.md`) simultaneously without opening a text editor.
* 🤖 **Dynamic Model Auto-Discovery:** query `/v1/models` across OpenAI-compatible providers with push-link logs per endpoint.
* 📝 **JSONC Comment Preservation:** parse/edit `opencode.json` / `opencode.jsonc` while preserving comments, trailing commas, and indentation byte-for-byte.
* 🔄 **Hot-Restart Automation:** automatically terminates and restarts `opencode` after successful save (current workflow; save is immediate).
* ⚙️ **Parameter Fine-Tuning:** `temperature`, `top_k`, `top_p`, `reasoning_effort`, `model` via intuitive TUI controls (`+/-` style on Form band).
* ⚡ **Native Rust Performance:** <30MB RAM; `ratatui` + `tokio`; byte-identical round-trip guarantees on both engines.

---

## Installation

### Prerequisites
* Rust toolchain (MSRV 1.75.0+)
* An active OpenCode installation (or any OpenAI-compatible provider; configured via `provider` in JSONC; otherwise config_defaults assumed)

### Building from Source
```bash
# Clone repository
git clone https://github.com/your-username/ocoger.git
cd ocoger

# Build + test + lint + format
cargo build --release
cargo test
cargo clippy   # zero-tolerance warnings
cargo fmt --check

# Optional: Install
cargo install --path .

```

### Verify
```bash
cargo run        # opens the TUI in ./.opencode/agents
```

---

## Keybindings Quick Reference

| Hotkey | Action Description |
|---|---|
| `j`/`k` or `↓`/`↑` | Navigate within list or form band |
| `Space` | Toggle selection checkmark on active agent |
| `a` | Toggle select-all / deselect-all |
| `m` | Enter `Mode::ModelEdit` (stage text with char-by-char editing) |
| `e`/`g`+Enter | Form band (params / global) switch per TODO |
| `Tab` | Switch bands in Form mode |
| `d` | **Unified diff preview** (staged mirror diff; review before applying) |
| `s`/`Ctrl+S` | **Save all changed agents + restart supervised process** (saves are atomic; process restart queued only after files are persisted) |
| `r` | Manual re-trigger of agent refresh (process restarts) |
| `q`/`Esc` | Exit/back — contextual bullet; within Modals apply/cancel routes it |

---

## Configuration Examples + Layout

### Agent memory (`.opencode/agents/<agent>.md`)
```yaml
---
model: deepseek-r1
temperature: 0.7
top_p: 0.9
reasoning_effort: high
---
You are an expert Rust backend engineer...
```

### Global config (`opencode.jsonc`)
```jsonc
{
  // primary provider + auth reference (should live in $OPENCODE_API_KEY in production)
  "model": "anthropic/claude-3-5-sonnet",
  "provider": "anthropic",
  "options": { "base_url": "https://api.anthropic.com" },
  "providers": [
    { "label": "openrouter", "options": { "base_url": "https://openrouter.ai/api/v1", "api_key": "${OPENROUTER_API_KEY}" } }
  ],
  "extra_body": { "max_tokens": 4096 }
}
```

### Project layout
```text
ocoger/
├── Cargo.toml
├── BRD.md
├── PRD.md
├── ARCH.md
├── ROADMAP.md
├── TODO.md
├── README.md
├── examples/
│   ├── jsonc_spike.rs      # validation pattern for jsonc edit bytes
│   └── proc_spike.rs       # Windows process-manager spike + behavior assertion
├── src/
│   ├── main.rs             # crossterm init + tray shadow
│   ├── core/
│   │   ├── agent_parser.rs
│   │   ├── agent_scanner.rs
│   │   ├── diff.rs         # unified_by_line + agent_diffs (pre-save preview)
│   │   ├── fs_util.rs      # atomic_write helper incl. Windows rename-overwrite
│   │   ├── jsonc_config.rs # JsoncConfig CST engine
│   │   └── mod.rs
│   ├── services/
│   │   ├── model_fetcher.rs # /v1/models fetcher with timeout + per-error report
│   │   ├── process_manager.rs
│   │   └── mod.rs
│   └── ui/
│       ├── app.rs         # App model (MVU) + pure tests
│       ├── event_handler.rs
│       └── widgets/
│           ├── agent_list.rs
│           ├── diff_view.rs  # unified diff rendering (green/red)
│           ├── form.rs
│           ├── log_drawer.rs
│           ├── model_picker.rs # static + fetched catalog visual
│           ├── picker.rs       # live-filter string modal
│           └── mod.rs
└── tmp/ (git-ignored)     # .ocoger/staging/ mirrors built dynamically at runtime

```

---

## License

Distributed under the MIT License. See `LICENSE` for details.
