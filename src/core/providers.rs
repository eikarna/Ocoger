//! Providers module: list providers from merged opencode.jsonc config.
//! Phase 5.3 - read/write provider entries via CST to preserve comments/formatting.

use serde_json::Value;
use thiserror::Error;

/// Information about a single provider as exposed in opencode.json(c).
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInfo {
    pub id: String,           // key under `provider:` map (e.g., "anthropic", "openai")
    pub name: Option<String>, // display name if present in provider name field
    pub base_url: Option<String>, // options.baseURL value (if present)
    pub has_api_key_ref: bool, // true if apiKey uses env reference like `{env:VAR}`
}

impl ProviderInfo {
    /// Extract provider info from the top-level `provider` object in JSONC.
    pub fn scan_providers(root: &Value) -> Result<Vec<Self>, ScanError> {
        let Some(obj) = root.as_object() else {
            return Ok(Vec::new());
        };

        // Look for `provider:` key at top level of merged config.
        let Some(provider_obj) = obj.get("provider").and_then(|v| v.as_object()) else {
            return Ok(Vec::new());
        };

        let mut providers = Vec::new();
        for (id, entry) in provider_obj.iter() {
            let mut pi = ProviderInfo {
                id: id.clone(),
                name: None,
                base_url: None,
                has_api_key_ref: false,
            };

            // Check for `name` field at top of provider entry.
            if let Some(name_val) = entry.get("name") {
                if let Some(n) = name_val.as_str() {
                    pi.name = Some(n.to_string());
                }
            }

            // Check `options.baseURL`.
            if let Some(options) = entry.get("options").and_then(|o| o.as_object()) {
                if let Some(base) = options.get("baseURL").and_then(|b| b.as_str()) {
                    pi.base_url = Some(base.to_string());
                }

                // Detect apiKey env references.
                if let Some(api_key) = options.get("apiKey") {
                    if let Some(s) = api_key.as_str() {
                        if s.contains("{env:") {
                            pi.has_api_key_ref = true;
                        }
                    }
                }
            }

            providers.push(pi);
        }

        providers.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(providers)
    }
}

#[derive(Error, Debug)]
pub enum ScanError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}
