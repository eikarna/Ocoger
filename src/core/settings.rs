//! Settings pane: top-level `theme`, `default_agent`, `autoupdate`, `share`
//! scan from merged config, edited via existing CST helpers.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsRow {
    pub key: String,
    pub value: String, // "" when missing; bools render as "true"/"false"
}

/// Known settings surfaces with a deterministic order for the pane.
pub const SETTINGS_KEYS: &[&str] = &["theme", "default_agent", "autoupdate", "share"];

pub fn scan(config: &Value) -> Vec<SettingsRow> {
    SETTINGS_KEYS
        .iter()
        .map(|k| {
            let v = config.get(*k).unwrap_or(&Value::Null);
            let value = match v {
                Value::Null => String::new(),
                Value::String(s) => s.clone(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            SettingsRow {
                key: (*k).to_string(),
                value,
            }
        })
        .collect()
}
