//! YAML frontmatter parse/mutate/write-back engine for `.opencode/agents/*.md`.
//!
//! Fidelity contract: the Markdown body and the raw YAML block survive a
//! parse -> serialize round-trip byte-identically. We never reserialize the
//! YAML (which would lose comments and key ordering); instead we splice the
//! original file at the frontmatter delimiters and return the original YAML
//! slice verbatim when unchanged.

use gray_matter::engine::YAML;
use gray_matter::Matter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Keys the MVU editor is allowed to mutate. Restricts key-injection.
pub const EDITABLE_KEYS: &[&str] = &["model", "temperature", "top_k", "top_p", "reasoning_effort"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFrontmatter {
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

#[derive(Debug, Clone)]
pub struct AgentFile {
    pub path: PathBuf,
    pub frontmatter: AgentFrontmatter,
    pub raw_body: String,
    pub is_selected: bool,
    /// Raw YAML slice (not in ARCH sketch; required for fidelity-preserving save).
    raw_yaml: String,
}

impl AgentFile {
    /// Apply frontmatter edits via the same splice path as `ParsedAgent`.
    pub fn update_models(&mut self, fields: &[(String, String)]) {
        let mut parsed = ParsedAgent {
            frontmatter: self.frontmatter.clone(),
            raw_yaml: self.raw_yaml.clone(),
            raw_body: self.raw_body.clone(),
        };
        parsed.update_models(fields);
        self.frontmatter = parsed.frontmatter;
        self.raw_yaml = parsed.raw_yaml;
    }

    /// Persist atomically (ARCH §4.1), preserving body + YAML fidelity.
    pub fn save(&self) -> std::io::Result<()> {
        let parsed = ParsedAgent {
            frontmatter: self.frontmatter.clone(),
            raw_yaml: self.raw_yaml.clone(),
            raw_body: self.raw_body.clone(),
        };
        super::fs_util::atomic_write(&self.path, &serialize_agent(&parsed))
    }
}

/// A parsed agent file that retains the original raw YAML slice for fidelity.
#[derive(Debug, Clone)]
pub struct ParsedAgent {
    pub frontmatter: AgentFrontmatter,
    /// The raw YAML text between the delimiters, exactly as on disk.
    pub raw_yaml: String,
    /// The Markdown body, byte-identical to the source.
    pub raw_body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("no YAML frontmatter block found")]
    NoFrontmatter,
    #[error("invalid agents directory IO: {0}")]
    Io(#[from] std::io::Error),
}

impl ParsedAgent {
    /// Surgically mutate keys in the raw YAML slice, preserving comment anchors.
    /// Values must be already-valid YAML scalar tokens; writes are unquoted.
    pub fn update_models(&mut self, fields: &[(String, String)]) {
        for (key, value) in fields {
            debug_assert!(
                EDITABLE_KEYS.contains(&key.as_str()),
                "key must be whitelisted"
            );
            // `value` group stops before an optional trailing `  # comment`.
            let pat = format!(r"(?m)^(\s*{key}\s*:\s*)([^\n#]+?)(\s+#.*)?$");
            let re = Regex::new(&pat).expect("static key regen");
            let raw = std::mem::take(&mut self.raw_yaml);
            let next = re
                .replace_all(&raw, |caps: &regex::Captures<'_>| {
                    format!(
                        "{}{}{}",
                        &caps[1],
                        value,
                        caps.get(3).map_or("", |c| c.as_str())
                    )
                })
                .into_owned();
            if next == raw {
                // Key was absent: append at the end of the block.
                let mut y = raw;
                if !y.is_empty() && !y.ends_with('\n') {
                    y.push('\n');
                }
                y.push_str(&format!("{key}: {value}\n"));
                self.raw_yaml = y.trim_end_matches('\n').to_string();
            } else {
                self.raw_yaml = next;
            }
            // Keep typed model in sync for state consumers.
            if key == "model" {
                self.frontmatter.model = value.clone();
            }
        }
    }

    /// Convenience single-key setter.
    pub fn set_model(&mut self, model: &str) {
        self.update_models(&[("model".into(), model.into())]);
    }
}

/// Read + parse an agent file into an editable view.
pub fn load_agent(path: &std::path::Path) -> Result<AgentFile, ParseError> {
    let content = std::fs::read_to_string(path)?;
    let parsed = parse_agent(&content)?;
    Ok(AgentFile {
        path: path.to_path_buf(),
        frontmatter: parsed.frontmatter,
        raw_yaml: parsed.raw_yaml,
        raw_body: parsed.raw_body,
        is_selected: false,
    })
}

/// Parse an agent markdown file into frontmatter + parts, preserving raw text.
///
/// Splits only at the leading `---` delimiters so the body is captured
/// verbatim. Uses `gray_matter` + `serde_yaml` for typed extraction.
pub fn parse_agent(content: &str) -> Result<ParsedAgent, ParseError> {
    let (raw_yaml, raw_body) = split_frontmatter(content).ok_or(ParseError::NoFrontmatter)?;

    let matter = Matter::<YAML>::new();
    let frontmatter = matter
        .parse_with_struct::<AgentFrontmatter>(content)
        .ok_or(ParseError::NoFrontmatter)?
        .data;

    Ok(ParsedAgent {
        frontmatter,
        raw_yaml: raw_yaml.to_string(),
        raw_body: raw_body.to_string(),
    })
}

/// Serialize back to a file string; edits performed via `update_models`
/// produce a byte-splice preserving comments and anchors.
pub fn serialize_agent(agent: &ParsedAgent) -> String {
    let mut out = String::with_capacity(agent.raw_yaml.len() + agent.raw_body.len() + 8);
    out.push_str("---\n");
    out.push_str(&agent.raw_yaml);
    // Ensure the YAML block ends with a newline before the closing delimiter.
    if !agent.raw_yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---");
    out.push_str(&agent.raw_body);
    out
}

/// Split `content` into (raw_yaml, body_after_closing_delimiter).
/// The body includes the newline immediately following the closing `---`.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let raw_yaml = &rest[..end]; // YAML without trailing newline
    let body = &rest[end + 4..]; // skip "\n---", keep remainder verbatim
                                 // body starts with the newline that followed the closing delimiter (if any)
    Some((raw_yaml, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture intentionally includes YAML comments and non-alphabetical key
    // order plus a Markdown body with `---` and `#` content that must be
    // preserved byte-for-byte.
    const FIXTURE: &str = r#"---
reasoning_effort: high   # keep this comment
model: anthropic/claude-3-5-sonnet
top_k: 40

# nested comment above a gap
temperature: 0.2
---
# System Prompt

You are a helpful assistant. --- inline divider.

- item 1
- item 2

```text
---
code fence containing delimiter
```
"#;

    #[test]
    fn round_trip_preserves_body_and_yaml_byte_for_byte() {
        let parsed = parse_agent(FIXTURE).expect("should parse");
        let out = serialize_agent(&parsed);
        assert_eq!(out, FIXTURE, "round-trip must be byte-identical");
    }

    #[test]
    fn frontmatter_fields_extracted() {
        let parsed = parse_agent(FIXTURE).expect("should parse");
        assert_eq!(parsed.frontmatter.model, "anthropic/claude-3-5-sonnet");
        assert_eq!(parsed.frontmatter.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(parsed.frontmatter.top_k, Some(40));
        assert_eq!(parsed.frontmatter.temperature, Some(0.2));
        assert_eq!(parsed.frontmatter.top_p, None);
    }

    #[test]
    fn set_model_surgical_edit_preserves_comment_columns() {
        let mut parsed = parse_agent(FIXTURE).expect("should parse");
        parsed.set_model("openai/gpt-4o");
        let out = serialize_agent(&parsed);
        let expected = FIXTURE.replace("anthropic/claude-3-5-sonnet", "openai/gpt-4o");
        assert_eq!(out, expected, "mutation must touch only the value token");
        assert_eq!(parsed.frontmatter.model, "openai/gpt-4o");
    }

    #[test]
    fn update_models_multi_key() {
        let mut parsed = parse_agent(FIXTURE).expect("should parse");
        parsed.update_models(&[
            ("model".into(), "opea/o1".into()),
            ("temperature".into(), "0.7".into()),
            ("top_k".into(), "12".into()),
        ]);
        let out = serialize_agent(&parsed);
        assert!(out.contains("model: opea/o1"));
        assert!(out.contains("temperature: 0.7"));
        assert!(out.contains("top_k: 12"));
        assert!(out.contains("# keep this comment"));
        assert!(out.contains("# nested comment above a gap"));
    }

    #[test]
    fn update_models_appends_missing_key() {
        let mut parsed = parse_agent(FIXTURE).expect("should parse");
        parsed.update_models(&[("top_p".into(), "0.95".into())]);
        let out = serialize_agent(&parsed);
        assert!(out.contains("top_p: 0.95"));
        // Comments and order must be preserved; the new key lands at the end.
        assert!(out.find("temperature: 0.2").unwrap() < out.find("top_p: 0.95").unwrap());
    }

    #[test]
    fn quoted_value_replacement_keeps_anchor_comment() {
        // Comment anchor at column boundary must survive.
        let src = "---\nmodel: pre  # pick the model\n---\nbody\n";
        let mut parsed = parse_agent(src).expect("parse");
        parsed.set_model("post");
        let out = serialize_agent(&parsed);
        assert_eq!(out, "---\nmodel: post  # pick the model\n---\nbody\n");
    }

    #[test]
    fn agent_file_save_round_trip_via_atomic_write() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-agentio-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("helper.md");
        std::fs::write(&path, FIXTURE).unwrap();

        // load -> mutate -> save
        let mut agent = load_agent(&path).expect("load");
        assert!(agent.path.ends_with("helper.md"));
        agent.update_models(&[("model".into(), "opea/o1".into())]);
        agent.save().expect("save");

        let disk = std::fs::read_to_string(&path).expect("read back");
        let expected = FIXTURE.replace("anthropic/claude-3-5-sonnet", "opea/o1");
        assert_eq!(
            disk, expected,
            "round trip through disk must stay byte-surgical"
        );
        assert!(!dir.join("helper.md.tmp").exists(), "tmp removed on rename");
    }
}
