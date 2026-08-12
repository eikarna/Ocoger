//! Permissions pane data: a flat, editable view of `permission.*` and
//! `agent.<name>.permission.*` from the OpenCode v1 schema.
//!
//! Values are either a verdict string or an object mapping glob/command
//! patterns to verdicts (<https://opencode.ai/docs/permissions>). Both shapes
//! are flattened into rows carrying their own CST keypath, so a pattern rule is
//! edited in place instead of being clobbered by a bare verdict.

use serde_json::Value;

/// Documented verdicts.
pub const VERDICTS: &[&str] = &["allow", "ask", "deny"];

/// Tool keys documented under `permission`. `edit` covers edit/write/patch.
pub const KNOWN: &[&str] = &[
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

/// One editable row. Tool rows and pattern rows share this shape so the pane
/// is a single flat list and every row knows exactly where it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermEntry {
    /// Full CST path, e.g. `["permission","bash"]` or
    /// `["permission","bash","rm *"]` or `["agent","build","permission","bash"]`.
    pub keypath: Vec<String>,
    /// Display label; pattern rows are indented by the renderer via `depth`.
    pub label: String,
    /// 0 = tool row, 1 = pattern row under a tool.
    pub depth: u8,
    /// Explicit verdict, or "" when unset.
    pub value: String,
    /// `Some(agent)` for a per-agent override row.
    pub agent: Option<String>,
    /// Where an unset value comes from: the documented default, a blanket
    /// `"permission": "x"`, or a `*` catch-all. Rendered dimmed.
    pub inherited: Option<String>,
    /// Number of pattern children (tool rows only), for the summary column.
    pub pattern_count: usize,
}

impl PermEntry {
    /// Verdict actually in effect for this row.
    pub fn effective(&self) -> String {
        if !self.value.is_empty() {
            return self.value.clone();
        }
        self.inherited.clone().unwrap_or_default()
    }

    /// A tool row holding pattern children must not be overwritten with a bare
    /// verdict — that would delete the user's rules.
    pub fn is_container(&self) -> bool {
        self.depth == 0 && self.pattern_count > 0
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

/// Cycle a verdict: allow -> ask -> deny -> allow. Unset starts at `ask`.
pub fn next_value(cur: &str) -> &'static str {
    match cur {
        "allow" => "ask",
        "ask" => "deny",
        "deny" => "allow",
        _ => "ask",
    }
}

/// Flatten one `permission` object into rows under `prefix`.
fn flatten_perm_obj(
    perm: &Value,
    prefix: &[&str],
    agent: Option<&str>,
    blanket: Option<&str>,
    out: &mut Vec<PermEntry>,
) {
    // `"permission": "deny"` — a bare verdict for everything.
    let obj = match perm {
        Value::Object(o) => o,
        _ => return,
    };
    let star = obj.get("*").and_then(Value::as_str);

    let mut tools: Vec<String> = KNOWN.iter().map(|s| s.to_string()).collect();
    for k in obj.keys() {
        if k != "*" && !tools.iter().any(|t| t == k) {
            tools.push(k.clone());
        }
    }
    tools.sort();

    for tool in tools {
        let node = obj.get(&tool);
        let mut keypath: Vec<String> = prefix.iter().map(|s| s.to_string()).collect();
        keypath.push(tool.clone());

        let (value, patterns) = match node {
            Some(Value::String(s)) => (s.clone(), Vec::new()),
            Some(Value::Object(map)) => (
                String::new(),
                map.iter()
                    .filter_map(|(pat, v)| v.as_str().map(|s| (pat.clone(), s.to_string())))
                    .collect::<Vec<_>>(),
            ),
            _ => (String::new(), Vec::new()),
        };

        let inherited = if !value.is_empty() || !patterns.is_empty() {
            None
        } else if let Some(b) = blanket.or(star) {
            Some(format!("{b} (inherited)"))
        } else if agent.is_some() {
            // Agent rows with nothing set fall back to the global config, which
            // the global row above already shows; don't invent a default here.
            None
        } else {
            Some(format!("{} (default)", documented_default(&tool)))
        };

        out.push(PermEntry {
            keypath: keypath.clone(),
            label: tool.clone(),
            depth: 0,
            value,
            agent: agent.map(str::to_string),
            inherited,
            pattern_count: patterns.len(),
        });

        // Pattern children, `*` first (it is the catch-all) then sorted.
        let mut patterns = patterns;
        patterns.sort_by(|a, b| match (a.0.as_str(), b.0.as_str()) {
            ("*", "*") => std::cmp::Ordering::Equal,
            ("*", _) => std::cmp::Ordering::Less,
            (_, "*") => std::cmp::Ordering::Greater,
            (x, y) => x.cmp(y),
        });
        for (pat, verdict) in patterns {
            let mut kp = keypath.clone();
            kp.push(pat.clone());
            out.push(PermEntry {
                keypath: kp,
                label: pat,
                depth: 1,
                value: verdict,
                agent: agent.map(str::to_string),
                inherited: None,
                pattern_count: 0,
            });
        }
    }
}

/// Build the pane rows: global `permission.*` followed by each on-disk agent's
/// `agent.<name>.permission.*` overrides.
pub fn scan(config: &Value, agent_names: &[String]) -> Vec<PermEntry> {
    let mut out = Vec::new();
    let perm = config.get("permission");
    let blanket = perm.and_then(Value::as_str);

    // A bare `"permission": "deny"` still produces tool rows so they can be
    // made explicit; they show the blanket verdict as inherited.
    let global = perm
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    flatten_perm_obj(&global, &["permission"], None, blanket, &mut out);

    if let Some(agents) = config.get("agent").and_then(Value::as_object) {
        for (name, cfg) in agents {
            if !agent_names.iter().any(|n| n == name) {
                continue;
            }
            let Some(p) = cfg.get("permission") else {
                continue;
            };
            if !p.is_object() {
                continue;
            }
            // Only emit rows the agent actually overrides — a full 13-tool
            // block per agent would bury the global list.
            let before = out.len();
            flatten_perm_obj(
                p,
                &["agent", name, "permission"],
                Some(name),
                None,
                &mut out,
            );
            out.truncate(before);
            if let Some(map) = p.as_object() {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for tool in keys {
                    if tool == "*" {
                        continue;
                    }
                    let sub = serde_json::json!({ tool.as_str(): map[tool].clone() });
                    let mut rows = Vec::new();
                    flatten_perm_obj(
                        &sub,
                        &["agent", name, "permission"],
                        Some(name),
                        None,
                        &mut rows,
                    );
                    // flatten emits all KNOWN tools; keep only this one's rows.
                    out.extend(rows.into_iter().filter(|r| {
                        r.keypath.get(3).map(String::as_str) == Some(tool.as_str())
                            && (r.depth == 1 || !r.value.is_empty())
                    }));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn find<'a>(rows: &'a [PermEntry], path: &[&str]) -> Option<&'a PermEntry> {
        rows.iter().find(|r| r.keypath == path)
    }

    #[test]
    fn tool_rows_cover_documented_keys_with_defaults() {
        let rows = scan(&json!({}), &[]);
        for tool in KNOWN {
            let r = find(&rows, &["permission", tool]).expect("row per documented tool");
            assert_eq!(r.value, "", "nothing configured");
            assert_eq!(
                r.effective(),
                format!("{} (default)", documented_default(tool))
            );
        }
        assert!(
            find(&rows, &["permission", "write"]).is_none(),
            "edit covers write"
        );
    }

    /// Glob tables are the reason the old pane couldn't edit anything: the rule
    /// map must become addressable rows, each with its own keypath.
    #[test]
    fn glob_tables_flatten_into_editable_pattern_rows() {
        let rows = scan(
            &json!({
                "permission": {
                    "bash": { "*": "ask", "rm -rf *": "deny", "git *": "allow" },
                    "read": "allow"
                }
            }),
            &[],
        );
        let bash = find(&rows, &["permission", "bash"]).unwrap();
        assert_eq!(bash.pattern_count, 3);
        assert!(bash.is_container(), "must refuse a bare-verdict overwrite");
        assert_eq!(bash.value, "", "the container itself has no verdict");

        let star = find(&rows, &["permission", "bash", "*"]).unwrap();
        assert_eq!(star.value, "ask");
        assert_eq!(star.depth, 1);
        let rm = find(&rows, &["permission", "bash", "rm -rf *"]).unwrap();
        assert_eq!(rm.value, "deny", "pattern is addressable by its exact text");

        // `*` sorts first among children.
        let kids: Vec<&str> = rows
            .iter()
            .filter(|r| r.depth == 1 && r.keypath[1] == "bash")
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(kids, ["*", "git *", "rm -rf *"]);

        let read = find(&rows, &["permission", "read"]).unwrap();
        assert!(!read.is_container());
        assert_eq!(read.value, "allow");
    }

    #[test]
    fn blanket_and_star_render_as_inherited() {
        let rows = scan(&json!({ "permission": "deny" }), &[]);
        let bash = find(&rows, &["permission", "bash"]).unwrap();
        assert_eq!(bash.value, "", "not written into the file");
        assert_eq!(bash.effective(), "deny (inherited)");

        let rows = scan(
            &json!({ "permission": { "*": "ask", "bash": "deny" } }),
            &[],
        );
        assert_eq!(find(&rows, &["permission", "bash"]).unwrap().value, "deny");
        assert_eq!(
            find(&rows, &["permission", "read"]).unwrap().effective(),
            "ask (inherited)"
        );
        assert!(
            !rows.iter().any(|r| r.label == "*" && r.depth == 0),
            "'*' is a modifier, not a tool row"
        );
    }

    #[test]
    fn agent_overrides_appear_only_for_on_disk_agents() {
        let rows = scan(
            &json!({
                "permission": { "bash": "ask" },
                "agent": {
                    "build": { "permission": { "bash": { "git *": "allow" }, "edit": "deny" } },
                    "ghost": { "permission": { "bash": "allow" } }
                }
            }),
            &["build".to_string()],
        );
        let ov = find(&rows, &["agent", "build", "permission", "edit"]).unwrap();
        assert_eq!(ov.value, "deny");
        assert_eq!(ov.agent.as_deref(), Some("build"));
        let pat = find(&rows, &["agent", "build", "permission", "bash", "git *"]).unwrap();
        assert_eq!(pat.value, "allow");
        assert!(
            !rows.iter().any(|r| r.agent.as_deref() == Some("ghost")),
            "agents without a file on disk are ignored"
        );
        // Tools the agent does not override must not be emitted for that agent.
        assert!(find(&rows, &["agent", "build", "permission", "grep"]).is_none());
    }

    #[test]
    fn next_value_cycles_documented_verdicts() {
        assert_eq!(next_value("allow"), "ask");
        assert_eq!(next_value("ask"), "deny");
        assert_eq!(next_value("deny"), "allow");
        assert_eq!(next_value(""), "ask");
    }
}
