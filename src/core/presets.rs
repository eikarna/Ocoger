//! Configuration presets (ROADMAP Phase 4).
//!
//! A preset is a named snapshot of `(model, temperature, top_k, top_p,
//! reasoning_effort)` — the agent `EDITABLE_KEYS` whitelist. Applying a preset
//! replays the same `(key, value)` tuples through `AgentFile::update_models`,
//! which performs comment-preserving byte-surgery on the frontmatter.
//!
//! Storage: `.ocoger/presets.jsonc` in the project root. Read-only via
//! `jsonc-parser` typed extraction (comments discarded on read); writes via
//! serde serialize -> `atomic_write`. Preset files are machine-owned (no user
//! comment requirement), so a full rewrite on save is acceptable.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

use super::fs_util::atomic_write;

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("failed to read presets file: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse presets JSONC: {0}")]
    Parse(String),
    #[error("presets root must be an object with a `presets: [...]` array")]
    NotAnObject,
}

/// One preset: name + display label + all EDITS as `(key, value)` strings so
/// they are replayed verbatim through `AgentFile::update_models`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl Preset {
    /// Rendered `(key, value)` tuples — the shape `AgentFile::update_models`
    /// expects. Only `Some` fields are emitted so a partial preset never
    /// touches keys it doesn't model.
    pub fn to_fields(&self) -> Vec<(String, String)> {
        let mut out = vec![("model".into(), self.model.clone())];
        if let Some(t) = self.temperature {
            out.push(("temperature".into(), format!("{t}")));
        }
        if let Some(t) = self.top_k {
            out.push(("top_k".into(), t.to_string()));
        }
        if let Some(t) = self.top_p {
            out.push(("top_p".into(), format!("{t}")));
        }
        if let Some(t) = &self.reasoning_effort {
            out.push(("reasoning_effort".into(), t.clone()));
        }
        out
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PresetFile {
    #[serde(default)]
    presets: Vec<Preset>,
}

/// In-memory view of `.ocoger/presets.jsonc`.
#[derive(Debug)]
pub struct Presets {
    pub path: PathBuf,
    pub items: Vec<Preset>,
}

impl Presets {
    fn path_for(project: &std::path::Path) -> PathBuf {
        project.join(".ocoger").join("presets.jsonc")
    }

    /// Load from disk, returning an empty preset list when the file does not
    /// exist. Parse errors propagate — the UI should log them.
    pub fn load(project: &std::path::Path) -> Result<Self, PresetError> {
        let path = Self::path_for(project);
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    items: Vec::new(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        let parsed: serde_json::Value =
            jsonc_parser::parse_to_serde_value(&raw, &jsonc_parser::ParseOptions::default())
                .map_err(|e| PresetError::Parse(e.to_string()))?;
        if !parsed.is_object() {
            return Err(PresetError::NotAnObject);
        }
        let file: PresetFile =
            serde_json::from_value(parsed).map_err(|e| PresetError::Parse(e.to_string()))?;
        Ok(Self {
            path,
            items: file.presets,
        })
    }

    /// Persist atomically. Creates the parent `.ocoger/` dir on first save.
    pub fn save(&self) -> Result<(), PresetError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = PresetFile {
            presets: self.items.clone(),
        };
        let s = serde_json::to_string_pretty(&file)
            .map_err(|e| PresetError::Parse(format!("serialize presets failed: {e}")))?;
        atomic_write(&self.path, &s)?;
        Ok(())
    }

    /// Append if `name` is new, overwrite otherwise.
    pub fn upsert(&mut self, p: Preset) {
        if let Some(slot) = self.items.iter_mut().find(|x| x.name == p.name) {
            *slot = p;
        } else {
            self.items.push(p);
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|p| p.name != name);
        self.items.len() != before
    }

    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.items.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-presets-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let dir = temp_root("missing");
        let p = Presets::load(&dir).unwrap();
        assert!(p.items.is_empty());
        assert!(p.path.ends_with("presets.jsonc"));
    }

    #[test]
    fn upsert_then_save_then_reload_round_trips() {
        let dir = temp_root("roundtrip");
        let mut ps = Presets::load(&dir).unwrap();
        ps.upsert(Preset {
            name: "deep-work".into(),
            description: Some("long reasoning runs".into()),
            model: "anthropic/claude-sonnet-4".into(),
            temperature: Some(0.3),
            top_k: None,
            top_p: Some(0.95),
            reasoning_effort: Some("high".into()),
        });
        ps.upsert(Preset {
            name: "fast-draft".into(),
            description: None,
            model: "openai/gpt-5-mini".into(),
            temperature: Some(0.7),
            top_k: Some(40),
            top_p: None,
            reasoning_effort: Some("low".into()),
        });
        ps.save().unwrap();

        let reload = Presets::load(&dir).unwrap();
        assert_eq!(reload.items.len(), 2);
        let dw = reload.get("deep-work").unwrap();
        assert_eq!(dw.model, "anthropic/claude-sonnet-4");
        assert_eq!(dw.temperature, Some(0.3));
        assert_eq!(dw.top_p, Some(0.95));
        assert_eq!(dw.reasoning_effort.as_deref(), Some("high"));
        let fd = reload.get("fast-draft").unwrap();
        assert_eq!(fd.top_k, Some(40));
        assert!(fd.top_p.is_none());
    }

    #[test]
    fn upsert_overwrites_existing_by_name() {
        let dir = temp_root("overwrite");
        let mut ps = Presets::load(&dir).unwrap();
        let mut p = Preset {
            name: "same".into(),
            description: None,
            model: "m1".into(),
            temperature: Some(0.1),
            top_k: None,
            top_p: None,
            reasoning_effort: None,
        };
        ps.upsert(p.clone());
        p.model = "m2".into();
        ps.upsert(p);
        assert_eq!(ps.items.len(), 1);
        assert_eq!(ps.get("same").unwrap().model, "m2");
    }

    #[test]
    fn remove_returns_true_only_when_present() {
        let dir = temp_root("remove");
        let mut ps = Presets::load(&dir).unwrap();
        ps.upsert(Preset {
            name: "x".into(),
            description: None,
            model: "m".into(),
            temperature: None,
            top_k: None,
            top_p: None,
            reasoning_effort: None,
        });
        assert!(ps.remove("x"));
        assert!(!ps.remove("x"));
    }

    #[test]
    fn to_fields_only_emits_some_fields() {
        let p = Preset {
            name: "p".into(),
            description: None,
            model: "m".into(),
            temperature: Some(0.5),
            top_k: None,
            top_p: Some(0.9),
            reasoning_effort: Some("medium".into()),
        };
        let fields = p.to_fields();
        assert_eq!(fields.len(), 4);
        let map: std::collections::HashMap<String, String> = fields.into_iter().collect();
        assert_eq!(map.get("model").map(String::as_str), Some("m"));
        assert_eq!(map.get("temperature").map(String::as_str), Some("0.5"));
        assert_eq!(map.get("top_k").map(String::as_str), None);
        assert_eq!(map.get("top_p").map(String::as_str), Some("0.9"));
        assert_eq!(
            map.get("reasoning_effort").map(String::as_str),
            Some("medium")
        );
    }

    #[test]
    fn save_creates_parent_dir_and_preserves_comments_in_read() {
        let dir = temp_root("comments");
        // Read path ignores comments but should not error.
        let target = dir.join(".ocoger").join("presets.jsonc");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(
            &target,
            r#"{
  // working set
  "presets": [
    { "name": "x", "model": "m", /* trailing */ "temperature": 0.5 }
  ]
}"#,
        )
        .unwrap();
        let ps = Presets::load(&dir).unwrap();
        assert_eq!(ps.items.len(), 1);
        assert_eq!(ps.items[0].name, "x");
        assert_eq!(ps.items[0].temperature, Some(0.5));
    }
}
