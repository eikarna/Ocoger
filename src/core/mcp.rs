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
                .get("url")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| match cfg.get("command") {
                    // Local servers usually store argv as an array.
                    Some(Value::Array(parts)) => Some(
                        parts
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                    Some(Value::String(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// MCP entries come from the merged config; remote servers carry `url`,
    /// local ones carry `command` (often an argv array, not a string).
    #[test]
    fn scan_reads_remote_url_and_local_command_array() {
        let cfg = json!({
            "mcp": {
                "context7": { "type": "remote", "url": "https://mcp.context7.com/mcp", "enabled": true },
                "serena": { "type": "local", "command": ["uvx", "serena", "mcp"], "enabled": false },
                "bare": {}
            }
        });
        let out = McpEntry::scan(&cfg);
        assert_eq!(out.len(), 3, "sorted by name");
        assert_eq!(out[0].name, "bare");
        assert_eq!(out[0].kind, "local", "type defaults to local");
        assert!(out[0].enabled, "enabled defaults to true");

        let c7 = out.iter().find(|e| e.name == "context7").unwrap();
        assert_eq!(c7.kind, "remote");
        assert_eq!(c7.command_or_url, "https://mcp.context7.com/mcp");

        let serena = out.iter().find(|e| e.name == "serena").unwrap();
        assert!(!serena.enabled);
        assert_eq!(
            serena.command_or_url, "uvx serena mcp",
            "argv arrays are joined for display"
        );
    }

    #[test]
    fn scan_without_mcp_key_is_empty() {
        assert!(McpEntry::scan(&json!({})).is_empty());
        assert!(McpEntry::scan(&Value::Null).is_empty());
    }
}
