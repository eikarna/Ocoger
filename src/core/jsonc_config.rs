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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigOrigin {
    /// From the file `save()` writes (project `opencode.jsonc`).
    Primary,
    /// Read-only display. Originates from a secondary source (typically the
    /// per-user global config). Never written back.
    GlobalReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    pub label: String,
    pub value: String,
    pub keypath: Vec<String>,
    #[serde(default)]
    pub origin: ConfigOrigin,
}

impl Default for ConfigOrigin {
    fn default() -> Self {
        Self::Primary
    }
}

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

    /// Load or create an empty-but-valid config for edit+save flows. Path is
    /// opencode.jsonc when neither file exists so save() writes a new file.
    pub fn ensure_loaded(project: &std::path::Path) -> Result<Self, ConfigError> {
        if let Some(c) = Self::load(project)? {
            return Ok(c);
        }
        let path = project.join("opencode.jsonc");
        let default_raw = "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n}\n".to_string();
        Ok(Self {
            path,
            raw: default_raw,
        })
    }

    /// Load from an explicit path (anywhere on disk). Used by the cascade
    /// resolver when the discovered primary path lies outside
    /// `<project>/opencode.jsonc`.
    pub fn load_at_path(path: &std::path::Path) -> Result<Option<Self>, ConfigError> {
        if !path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(path)?;
        parse_cst(&raw)?;
        Ok(Some(Self {
            path: path.to_path_buf(),
            raw,
        }))
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

    /// Surgical nested-path string mutation (only first-level nesting supported;
    /// MVU only navigates to `provider.<name>.options.baseURL` and `provider.<name>.api_key`).
    pub fn set_nested_str<I, S>(&mut self, path: I, value: &str) -> Result<(), ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut path: Vec<String> = path.into_iter().map(|s| s.as_ref().to_owned()).collect();
        if path.is_empty() {
            return Err(ConfigError::NotAnObject);
        }
        let root = parse_cst(&self.raw)?;
        let mut obj = root.object_value_or_set();
        let leaf = path.pop().unwrap();
        for seg in path {
            match obj.object_value_or_create(&seg) {
                Some(o) => obj = o,
                None => return Err(ConfigError::NotAnObject),
            }
        }
        match obj.get(&leaf) {
            Some(prop) => prop.set_value(CstInputValue::String(value.to_owned())),
            None => {
                obj.append(&leaf, CstInputValue::String(value.to_owned()));
            }
        }
        self.raw = root.to_string();
        Ok(())
    }

    /// Surgical nested-path value mutation (string/bool/number) preserving
    /// comments. `Bool`/`Number` survive as typed values; use for `enabled`
    /// toggles & enum-ish fields like ask/allow/deny.
    pub fn set_nested_value<I, S>(&mut self, path: I, val: CstInputValue) -> Result<(), ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut path: Vec<String> = path.into_iter().map(|s| s.as_ref().to_owned()).collect();
        if path.is_empty() {
            return Err(ConfigError::NotAnObject);
        }
        let root = parse_cst(&self.raw)?;
        let mut obj = root.object_value_or_set();
        let leaf = path.pop().unwrap();
        for seg in path {
            match obj.object_value_or_create(&seg) {
                Some(o) => obj = o,
                None => return Err(ConfigError::NotAnObject),
            }
        }
        match obj.get(&leaf) {
            Some(prop) => prop.set_value(val),
            None => {
                obj.append(&leaf, val);
            }
        }
        self.raw = root.to_string();
        Ok(())
    }

    /// Surgical removal of a top-level or nested key, preserving everything
    /// around it. Used by MCP/provider delete flows.
    pub fn remove_key(&mut self, key: &str) -> Result<(), ConfigError> {
        let root = parse_cst(&self.raw)?;
        let obj = root.object_value_or_set();
        if let Some(prop) = obj.get(key) {
            prop.remove();
        }
        self.raw = root.to_string();
        Ok(())
    }

    /// Nested-path removal: deletes `path` leaf from its parent object.
    pub fn remove_nested<I, S>(&mut self, path: I) -> Result<(), ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut path: Vec<String> = path.into_iter().map(|s| s.as_ref().to_owned()).collect();
        if path.is_empty() {
            return Err(ConfigError::NotAnObject);
        }
        let root = parse_cst(&self.raw)?;
        let mut obj = root.object_value_or_set();
        let leaf = path.pop().unwrap();
        for seg in path {
            match obj.object_value_or_create(&seg) {
                Some(o) => obj = o,
                None => return Ok(()), // parent missing → nothing to delete
            }
        }
        if let Some(prop) = obj.get(&leaf) {
            prop.remove();
        }
        self.raw = root.to_string();
        Ok(())
    }

    /// One row display in the global config pane.
    /// Extract the form fields the PRD's global pane cares about.
    pub fn config_items(&self) -> Result<Vec<ConfigItem>, ConfigError> {
        let v = self.value()?;
        let mut out = Vec::new();
        let mut push = |label: &str, value: Option<&str>, keypath: &[&str]| {
            out.push(ConfigItem {
                label: label.into(),
                value: value.unwrap_or("").into(),
                keypath: keypath.iter().map(|s| s.to_string()).collect(),
                origin: ConfigOrigin::Primary,
            });
        };
        push("model", v.get("model").and_then(Value::as_str), &["model"]);
        push(
            "small_model",
            v.get("small_model").and_then(Value::as_str),
            &["small_model"],
        );
        push("theme", v.get("theme").and_then(Value::as_str), &["theme"]);
        push(
            "default_agent",
            v.get("default_agent").and_then(Value::as_str),
            &["default_agent"],
        );
        let provider = v
            .get("provider")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        if let Some(map) = provider.as_object() {
            for (name, cfg) in map {
                // v1 schema nests both under `options`; `api_key` at the entry
                // root was never valid and always read back empty.
                push(
                    &format!("provider.{name}.baseURL"),
                    cfg.pointer("/options/baseURL").and_then(Value::as_str),
                    &["provider", name, "options", "baseURL"],
                );
                push(
                    &format!("provider.{name}.apiKey"),
                    cfg.pointer("/options/apiKey").and_then(Value::as_str),
                    &["provider", name, "options", "apiKey"],
                );
            }
        }
        Ok(out)
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
    fn config_items_extract_top_level_and_provider() {
        let cfg = JsoncConfig {
            path: PathBuf::from("opencode.jsonc"),
            raw: r#"{
  "model": "oclaude/claude-3-5",  // primary
  "theme": "dark",
  "provider": {
    "openai": { "options": { "baseURL": "https://api.openai.com", "apiKey": "sk-xxx" } },
    "anthropic": { "options": { "baseURL": "https://api.anthropic.com" } },
  },
}"#
            .to_string(),
        };
        let items = cfg.config_items().unwrap();
        let get = |label: &str| items.iter().find(|i| i.label == label).unwrap();
        assert_eq!(get("model").value, "oclaude/claude-3-5");
        assert_eq!(get("theme").value, "dark");
        assert_eq!(
            get("provider.openai.baseURL").value,
            "https://api.openai.com"
        );
        assert_eq!(
            get("provider.openai.apiKey").value,
            "sk-xxx",
            "v1 schema nests apiKey under options"
        );
        assert_eq!(
            get("provider.anthropic.baseURL").value,
            "https://api.anthropic.com"
        );
        assert_eq!(get("provider.anthropic.apiKey").value, "");
    }

    #[test]
    fn set_nested_str_mutates_target_and_preserves_siblings() {
        let mut cfg = JsoncConfig {
            path: PathBuf::from("opencode.jsonc"),
            raw: r#"{
  "model": "old",     // primary comment
  "provider": {
    "openai": {
      // comment inside
      "options": { "baseURL": "https://api.openai.com" },
    },
  },
}"#
            .to_string(),
        };
        cfg.set_nested_str(
            ["provider", "openai", "options", "baseURL"],
            "https://openrouter.ai/api/v1",
        )
        .unwrap();
        cfg.set_nested_str(["openai", "api_key"], "sk-new").unwrap();
        let s = cfg.raw;
        assert!(s.contains("\"baseURL\": \"https://openrouter.ai/api/v1\""));
        assert!(s.contains("// primary comment"), "top comment preserved");
        assert!(s.contains("// comment inside"), "sibling comment preserved");
        assert!(s.contains("\"api_key\": \"sk-new\""));
    }

    #[test]
    fn ensure_loaded_returns_default_and_save_creates_file() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-cfg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            JsoncConfig::load(&dir).unwrap().is_none(),
            "no config present"
        );
        let cfg = JsoncConfig::ensure_loaded(&dir).unwrap();
        assert_eq!(
            cfg.value().unwrap().get("$schema").and_then(Value::as_str),
            Some("https://opencode.ai/config.json")
        );
        cfg.save().unwrap();
        let re = JsoncConfig::load(&dir).unwrap().expect("exists");
        assert_eq!(
            re.path.file_name().unwrap().to_string_lossy(),
            "opencode.jsonc"
        );
        let on_disk = std::fs::read_to_string(re.path).unwrap();
        assert!(on_disk.contains("https://opencode.ai/config.json"));
    }
}
