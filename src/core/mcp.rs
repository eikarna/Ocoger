//! MCP servers (`mcp.<name>`) — read/write via JSONC CST.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEntry {
    pub name: String,
    pub kind: String, // "local" | "remote" | other
    pub command_or_url: String,
    pub enabled: bool,
}

impl McpEntry {
    pub fn scan(config: &Value) -> Vec<Self> {
        let mut out = Vec::new();
        let Some(map) = config.get("mcp").and_then(Value::as_object) else {
            return out;
        };
        for (name, cfg) in map {
            let kind = cfg
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("local")
                .to_string();
            let command_or_url = cfg
                .get("command")
                .and_then(Value::as_str)
                .or_else(|| cfg.get("url").and_then(Value::as_str))
                .unwrap_or("")
                .to_string();
            let enabled = cfg.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            out.push(Self {
                name: name.clone(),
                kind,
                command_or_url,
                enabled,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}
