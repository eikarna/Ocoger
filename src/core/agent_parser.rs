//! YAML frontmatter parse/mutate/write-back engine for `.opencode/agents/*.md`.
//!
//! Fidelity contract: the Markdown body and the raw YAML block survive a
//! parse -> serialize round-trip byte-identically. We never reserialize the
//! YAML (which would lose comments and key ordering); instead we splice the
//! original file at the frontmatter delimiters and return the original YAML
//! slice verbatim when unchanged.

use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Serialize back to a file string. If `raw_yaml` is unchanged we reproduce
/// the original byte-for-byte; otherwise the (edited) YAML is re-emitted.
///
/// For now we always emit with `raw_yaml` preserved (no mutation support yet).
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
}
