//! Reader/writer for `opencode.json` / `opencode.jsonc`.
//!
//! Strategy (spike-validated, TODO §1): `jsonc-parser` CST edits via
//! `object_value_or_set() -> get(key) -> prop.set_value(...)` perform surgical
//! single-key mutations preserving 100% of comments and formatting
//! byte-for-byte. Never round-trip through serde for writes — it strips
//! comments.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;

use super::fs_util::atomic_write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub extra_body: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse JSONC: {0}")]
    Parse(String),
    #[error("root of config is not an object")]
    NotAnObject,
}

/// Located JSONC config file, preferring `opencode.jsonc` over `opencode.json`.
pub struct JsoncConfig {
    /// Which file exists (or would be written), first match wins.
    pub path: PathBuf,
    raw: String,
}

impl JsoncConfig {
    /// Load `<project>/opencode.jsonc`, falling back to `opencode.json`.
    /// Returns `None` when neither file exists.
    pub fn load(project: &std::path::Path) -> Result<Option<Self>, ConfigError> {
        for name in ["opencode.jsonc", "opencode.json"] {
            let path = project.join(name);
            if path.is_file() {
                let raw = std::fs::read_to_string(&path)?;
                // Parse once now so the UI can flag broken config immediately.
                parse_cst(&raw)?;
                return Ok(Some(Self { path, raw }));
            }
        }
        Ok(None)
    }

    /// Typed read-only view of the whole config (comments discarded on read).
    pub fn value(&self) -> Result<Value, ConfigError> {
        let parsed: Value =
            jsonc_parser::parse_to_serde_value::<Value>(&self.raw, &ParseOptions::default())
                .map_err(|e| ConfigError::Parse(e.to_string()))?;
        if parsed.is_object() {
            Ok(parsed)
        } else {
            Err(ConfigError::NotAnObject)
        }
    }

    /// Current top-level `model` value, if any.
    pub fn model(&self) -> Result<Option<String>, ConfigError> {
        Ok(self
            .value()?
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned))
    }

    /// Set the top-level `model` string via a surgical CST text edit.
    pub fn set_model(&mut self, model: &str) -> Result<(), ConfigError> {
        self.set_top_level_str("model", model)
    }

    /// Generic surgical single-key string edit preserving comments/formatting.
    pub fn set_top_level_str(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        let root = parse_cst(&self.raw)?;
        let obj = root.object_value_or_set();
        match obj.get(key) {
            Some(prop) => prop.set_value(CstInputValue::String(value.to_owned())),
            None => {
                obj.append(key, CstInputValue::String(value.to_owned()));
            }
        }
        self.raw = root.to_string();
        Ok(())
    }

    /// Persist with the atomic temp-then-rename pipeline (ARCH §4.1).
    pub fn save(&self) -> Result<(), ConfigError> {
        Ok(atomic_write(&self.path, &self.raw)?)
    }
}

fn parse_cst(raw: &str) -> Result<CstRootNode, ConfigError> {
    CstRootNode::parse(raw, &ParseOptions::default()).map_err(|e| ConfigError::Parse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"// header comment
{
  "$schema": "https://opencode.ai/config.json",
  "model": "anthropic/claude-3-5-sonnet", // primary model
  /* trailing block */
}
"#;

    #[test]
    fn model_round_trip() {
        let mut cfg = JsoncConfig {
            path: PathBuf::from("opencode.jsonc"),
            raw: FIXTURE.to_string(),
        };
        assert_eq!(
            cfg.model().unwrap().as_deref(),
            Some("anthropic/claude-3-5-sonnet")
        );
        cfg.set_model("openai/gpt-4o").unwrap();
        assert_eq!(cfg.model().unwrap().as_deref(), Some("openai/gpt-4o"));
    }

    #[test]
    fn comments_and_formatting_survive_mutation() {
        let mut cfg = JsoncConfig {
            path: PathBuf::from("opencode.jsonc"),
            raw: FIXTURE.to_string(),
        };
        cfg.set_model("openai/gpt-4o").unwrap();
        let expected = FIXTURE.replace("anthropic/claude-3-5-sonnet", "openai/gpt-4o");
        assert_eq!(cfg.raw, expected, "mutation must be byte-surgical");
    }

    #[test]
    fn set_missing_key_appends_preserving_comments() {
        let mut cfg = JsoncConfig {
            path: PathBuf::from("opencode.jsonc"),
            raw: FIXTURE.to_string(),
        };
        cfg.set_top_level_str("theme", "dark").unwrap();
        assert!(cfg.raw.contains("// header comment"));
        assert!(cfg.raw.contains("/* trailing block */"));
        assert_eq!(
            cfg.value().unwrap().get("theme").and_then(Value::as_str),
            Some("dark")
        );
        // Original model untouched.
        assert_eq!(
            cfg.model().unwrap().as_deref(),
            Some("anthropic/claude-3-5-sonnet")
        );
    }

    #[test]
    fn invalid_jsonc_is_rejected() {
        assert!(parse_cst("{\n  \"model\": ,\n}").is_err());
    }
}
