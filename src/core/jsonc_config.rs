//! Reader/writer for `opencode.json` / `opencode.jsonc`.
//!
//! Strategy (spike-validated, TODO §1): `jsonc-parser` CST edits via
//! `object_value_or_set() -> get(key) -> prop.set_value(...)` perform surgical
//! single-key mutations preserving 100% of comments and formatting
//! byte-for-byte. Never round-trip through serde — it strips comments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub extra_body: Option<serde_json::Value>,
}
