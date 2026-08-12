//! Permissions pane data: merge `permission.<tool>` (global value) with
//! `agent.<name>.permission.<tool>` overrides into a single perm matrix.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One permission row: tool name → global value + map of agent → override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermRow {
    pub tool: String,
    pub global: String, // "ask" | "allow" | "deny" | ""
    pub agent_overrides: BTreeMap<String, String>,
}

const KNOWN: &[&str] = &[
    "read", "edit", "write", "glob", "grep", "bash", "task", "webfetch", "skill",
];

/// Render a permission value. OpenCode allows either a bare verdict
/// (`"bash": "ask"`) or a glob table (`"bash": { "rm *": "deny", "*": "ask" }`).
/// For a table, show the `*` fallback plus the rule count so the pane conveys
/// the shape without needing a nested editor.
fn render_value(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(map)) => {
            let fallback = map.get("*").and_then(Value::as_str).unwrap_or("-");
            let extra = map.len().saturating_sub(usize::from(map.contains_key("*")));
            if extra == 0 {
                fallback.to_string()
            } else {
                format!("{fallback} +{extra} glob")
            }
        }
        _ => String::new(),
    }
}

pub fn scan(config: &Value, agent_names: &[String]) -> Vec<PermRow> {
    let perm_obj = config.get("permission").and_then(Value::as_object);
    let agent_obj = config.get("agent").and_then(Value::as_object);

    let mut tools: BTreeSet<String> = KNOWN.iter().map(|s| s.to_string()).collect();
    if let Some(map) = perm_obj {
        for t in map.keys() {
            tools.insert(t.clone());
        }
    }

    let mut out: Vec<PermRow> = tools
        .into_iter()
        .map(|tool| PermRow {
            global: render_value(perm_obj.and_then(|m| m.get(&tool))),
            tool,
            agent_overrides: BTreeMap::new(),
        })
        .collect();

    if let Some(agents_map) = agent_obj {
        for (name, cfg) in agents_map {
            if !agent_names.iter().any(|n| n == name) {
                continue;
            }
            if let Some(p) = cfg.get("permission").and_then(Value::as_object) {
                for (tool, v) in p {
                    if let Some(row) = out.iter_mut().find(|r| &r.tool == tool) {
                        let rendered = render_value(Some(v));
                        if !rendered.is_empty() {
                            row.agent_overrides.insert(name.clone(), rendered);
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn next_value(cur: &str) -> &'static str {
    match cur {
        "ask" => "allow",
        "allow" => "deny",
        _ => "ask",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Panes read the *merged* config, where `permission` values may be a bare
    /// verdict or a glob table. Both must render; a table previously produced an
    /// empty cell because only `as_str()` was consulted.
    #[test]
    fn scan_renders_string_and_glob_table_permissions() {
        let cfg = json!({
            "permission": {
                "read": "allow",
                "bash": { "*": "ask", "rm *": "deny", "rm -rf *": "deny" },
                "edit": { "src/**": "allow" }
            }
        });
        let rows = scan(&cfg, &[]);
        let get = |t: &str| {
            rows.iter()
                .find(|r| r.tool == t)
                .map(|r| r.global.clone())
                .unwrap_or_default()
        };
        assert_eq!(get("read"), "allow");
        assert_eq!(get("bash"), "ask +2 glob", "fallback plus rule count");
        assert_eq!(get("edit"), "- +1 glob", "no '*' entry → '-' fallback");
        assert_eq!(get("webfetch"), "", "unset tool stays blank");
    }

    #[test]
    fn scan_collects_per_agent_overrides_for_known_agents() {
        let cfg = json!({
            "permission": { "bash": "ask" },
            "agent": {
                "reviewer": { "permission": { "bash": "deny" } },
                "stranger": { "permission": { "bash": "allow" } }
            }
        });
        let rows = scan(&cfg, &["reviewer".to_string()]);
        let bash = rows.iter().find(|r| r.tool == "bash").unwrap();
        assert_eq!(
            bash.agent_overrides.get("reviewer").map(String::as_str),
            Some("deny")
        );
        assert!(
            !bash.agent_overrides.contains_key("stranger"),
            "agents not present on disk are ignored"
        );
    }
}
