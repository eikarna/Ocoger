//! Reader/writer for `opencode.json` / `opencode.jsonc`.
// TODO(P1): JSONC comment-preservation spike — `serde_json5` strips comments.
// See TODO.md; this is a hard KPI (100% comment retention).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub extra_body: Option<serde_json::Value>,
}
