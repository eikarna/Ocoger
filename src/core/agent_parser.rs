//! YAML frontmatter parse/mutate/write engine for `.opencode/agents/*.md`.
// TODO(P0): round-trip test — Markdown body must survive byte-identical.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub struct AgentFile {
    pub path: PathBuf,
    pub frontmatter: AgentFrontmatter,
    pub raw_body: String,
    pub is_selected: bool,
}
