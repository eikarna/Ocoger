//! Settings pane: top-level scalar settings from the OpenCode v1 config schema
//! (<https://opencode.ai/docs/config>), edited via the JSONC CST helpers.

use serde_json::Value;

/// How a setting is edited. Drives both rendering and the Space/Enter handlers
/// so the pane needs no per-key special cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Free text, e.g. `model`, `theme`.
    Text,
    /// `true` / `false`.
    Bool,
    /// `true` / `false` / `"notify"` — `autoupdate` only.
    BoolOrNotify,
    /// Fixed set of string literals, cycled in order.
    Enum(&'static [&'static str]),
    /// Integer.
    Number,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsRow {
    pub key: String,
    /// "" when the key is absent from the merged config.
    pub value: String,
    pub kind: Kind,
    /// Documented default, shown dimmed when the key is unset.
    pub default: &'static str,
    pub hint: &'static str,
}

/// `share` accepts exactly these (docs → Sharing). Default `manual`.
pub const SHARE_VALUES: &[&str] = &["manual", "auto", "disabled"];

/// Known top-level scalar settings, in pane order.
/// Deliberately excludes container keys (`provider`, `mcp`, `permission`,
/// `agent`, `command`, `formatter`, `lsp`, `plugin`, `instructions`,
/// `experimental`) — those have their own panes or are not scalar-editable.
const KNOWN: &[(&str, Kind, &str, &str)] = &[
    (
        "model",
        Kind::Text,
        "",
        "provider/model, e.g. anthropic/claude-sonnet-4-5",
    ),
    ("small_model", Kind::Text, "", "cheap model for titles etc."),
    (
        "default_agent",
        Kind::Text,
        "build",
        "primary agent name (not a subagent)",
    ),
    (
        "share",
        Kind::Enum(SHARE_VALUES),
        "manual",
        "manual | auto | disabled",
    ),
    (
        "autoupdate",
        Kind::BoolOrNotify,
        "true",
        "true | false | notify",
    ),
    ("snapshot", Kind::Bool, "true", "false disables rollback"),
    (
        "subagent_depth",
        Kind::Number,
        "1",
        "0 = no subagents, 2 = one nested level",
    ),
    (
        "shell",
        Kind::Text,
        "",
        "short name or absolute path, e.g. pwsh",
    ),
    // `theme` belongs in OpenCode's tui.json, not its v1 config schema.
    // Ocoger's visual theme is stored separately under `.ocoger/`; never put
    // it here or create an accidental project opencode.jsonc override.
];

/// Render a scalar JSON value for display. Non-scalars collapse to "" so a
/// mistyped container never renders as a blob of JSON.
fn render(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn scan(config: &Value) -> Vec<SettingsRow> {
    KNOWN
        .iter()
        .map(|(key, kind, default, hint)| SettingsRow {
            value: render(config.get(*key)),
            key: (*key).to_string(),
            kind: *kind,
            default,
            hint,
        })
        .collect()
}

/// Next value when the row is toggled/cycled. `None` means the row is
/// free-text and must go through the input modal instead.
pub fn cycle(row: &SettingsRow) -> Option<String> {
    let cur = row.value.as_str();
    match row.kind {
        Kind::Text => None,
        Kind::Bool => Some(if cur == "true" { "false" } else { "true" }.to_string()),
        // Documented union: true -> false -> notify -> true.
        Kind::BoolOrNotify => Some(
            match cur {
                "true" => "false",
                "false" => "notify",
                _ => "true",
            }
            .to_string(),
        ),
        Kind::Enum(vals) => {
            let idx = vals
                .iter()
                .position(|v| *v == cur)
                .map(|i| i + 1)
                .unwrap_or(0);
            Some(vals[idx % vals.len()].to_string())
        }
        Kind::Number => {
            let n: i64 = cur.parse().unwrap_or(-1);
            // subagent_depth is the only number here; 0..=3 covers documented use.
            Some(((n + 1).clamp(0, 3) % 4).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(rows: &[SettingsRow], key: &str) -> SettingsRow {
        rows.iter().find(|r| r.key == key).unwrap().clone()
    }

    #[test]
    fn scan_renders_scalars_and_leaves_absent_keys_blank() {
        let rows = scan(&json!({
            "model": "anthropic/claude-sonnet-4-5",
            "autoupdate": false,
            "subagent_depth": 2,
            "share": "auto",
            // Containers must not leak into the scalar pane.
            "provider": { "anthropic": {} }
        }));
        assert_eq!(row(&rows, "model").value, "anthropic/claude-sonnet-4-5");
        assert_eq!(
            row(&rows, "autoupdate").value,
            "false",
            "bools render as literals"
        );
        assert_eq!(row(&rows, "subagent_depth").value, "2");
        assert_eq!(row(&rows, "share").value, "auto");
        assert_eq!(row(&rows, "shell").value, "", "absent key is blank");
        assert!(
            !rows.iter().any(|r| r.key == "provider"),
            "container keys belong to their own panes"
        );
    }

    /// The documented `share` enum is manual/auto/disabled — not on/off/ask.
    #[test]
    fn share_cycles_through_documented_literals_only() {
        let rows = scan(&json!({}));
        let mut r = row(&rows, "share");
        let mut seen = Vec::new();
        for _ in 0..4 {
            let next = cycle(&r).unwrap();
            seen.push(next.clone());
            r.value = next;
        }
        assert_eq!(seen, ["manual", "auto", "disabled", "manual"]);
    }

    /// `autoupdate` is a bool|"notify" union, so a plain bool flip is wrong.
    #[test]
    fn autoupdate_cycles_true_false_notify() {
        let rows = scan(&json!({ "autoupdate": true }));
        let mut r = row(&rows, "autoupdate");
        let mut seen = Vec::new();
        for _ in 0..3 {
            let next = cycle(&r).unwrap();
            seen.push(next.clone());
            r.value = next;
        }
        assert_eq!(seen, ["false", "notify", "true"]);
    }

    #[test]
    fn text_rows_have_no_cycle_and_bools_flip() {
        let rows = scan(&json!({ "snapshot": true }));
        assert_eq!(
            cycle(&row(&rows, "model")),
            None,
            "text needs the input modal"
        );
        assert_eq!(cycle(&row(&rows, "snapshot")).unwrap(), "false");
    }
}
