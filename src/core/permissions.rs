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
            global: perm_obj
                .and_then(|m| m.get(&tool))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
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
                        if let Some(s) = v.as_str() {
                            row.agent_overrides.insert(name.clone(), s.to_string());
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
