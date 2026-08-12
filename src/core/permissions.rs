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

/// Tool keys documented under `permission` (<https://opencode.ai/docs/permissions>).
/// `edit` covers edit/write/patch; there are no separate keys for those.
const KNOWN: &[&str] = &[
    "read",
    "edit",
    "glob",
    "grep",
    "bash",
    "task",
    "skill",
    "lsp",
    "question",
    "webfetch",
    "websearch",
    "external_directory",
    "doom_loop",
];

/// Documented verdicts. `permission` values are one of these, or an object
/// mapping patterns to one of these.
pub const VERDICTS: &[&str] = &["allow", "ask", "deny"];

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
    // `permission` may itself be a bare verdict string applying to everything.
    let blanket = config.get("permission").and_then(Value::as_str);
    let perm_obj = config.get("permission").and_then(Value::as_object);
    let agent_obj = config.get("agent").and_then(Value::as_object);
    // A top-level "*" inside the permission object is the catch-all default.
    let star = perm_obj.and_then(|m| m.get("*")).and_then(Value::as_str);

    let mut tools: BTreeSet<String> = KNOWN.iter().map(|s| s.to_string()).collect();
    if let Some(map) = perm_obj {
        for t in map.keys() {
            if t != "*" {
                tools.insert(t.clone());
            }
        }
    }

    let mut out: Vec<PermRow> = tools
        .into_iter()
        .map(|tool| {
            let explicit = render_value(perm_obj.and_then(|m| m.get(&tool)));
            let global = if !explicit.is_empty() {
                explicit
            } else if let Some(b) = blanket.or(star) {
                // Inherited from `"permission": "deny"` or `permission."*"`.
                format!("{b} *")
            } else {
                String::new()
            };
            PermRow {
                global,
                tool,
                agent_overrides: BTreeMap::new(),
            }
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

/// Cycle a verdict: allow -> ask -> deny -> allow. Anything unrecognised
/// (unset, or a glob table) starts at `ask`, the safe middle ground.
pub fn next_value(cur: &str) -> &'static str {
    match cur {
        "allow" => "ask",
        "ask" => "deny",
        "deny" => "allow",
        _ => "ask",
    }
}

/// Documented default verdict when a key is absent. Most tools default to
/// `allow`; `external_directory` and `doom_loop` default to `ask`.
pub fn documented_default(tool: &str) -> &'static str {
    match tool {
        "external_directory" | "doom_loop" => "ask",
        _ => "allow",
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

    #[test]
    fn blanket_string_and_star_are_shown_as_inherited() {
        // `"permission": "deny"` applies to everything.
        let rows = scan(&json!({ "permission": "deny" }), &[]);
        assert_eq!(
            rows.iter().find(|r| r.tool == "bash").unwrap().global,
            "deny *",
            "blanket verdict marked as inherited, not as an explicit bash rule"
        );

        // A top-level "*" is the catch-all; explicit keys still win.
        let rows = scan(
            &json!({ "permission": { "*": "ask", "bash": "deny" } }),
            &[],
        );
        let get = |t: &str| {
            rows.iter()
                .find(|r| r.tool == t)
                .map(|r| r.global.clone())
                .unwrap_or_default()
        };
        assert_eq!(get("bash"), "deny", "explicit key overrides the catch-all");
        assert_eq!(get("read"), "ask *", "unset key inherits '*'");
        assert!(
            !rows.iter().any(|r| r.tool == "*"),
            "'*' is a modifier, not a tool row"
        );
    }

    /// Docs: allow -> ask -> deny. Unset/glob-table rows start at `ask`.
    #[test]
    fn next_value_cycles_documented_verdicts() {
        assert_eq!(next_value("allow"), "ask");
        assert_eq!(next_value("ask"), "deny");
        assert_eq!(next_value("deny"), "allow");
        assert_eq!(next_value(""), "ask");
        assert_eq!(next_value("ask +2 glob"), "ask");
    }

    #[test]
    fn documented_defaults_match_docs() {
        assert_eq!(documented_default("bash"), "allow");
        assert_eq!(documented_default("external_directory"), "ask");
        assert_eq!(documented_default("doom_loop"), "ask");
    }
}
