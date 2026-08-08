//! Non-blocking async fetcher for provider `/v1/models` catalogs.
//!
//! Per ARCH §2.3: parse provider base URLs from `opencode.jsonc`, spawn tokio
//! tasks, GET `/v1/models` with Bearer auth, dedupe, publish into a shared
//! catalog. Adds a static Anthropic fallback when the endpoint is native and
//! no key is configured.

use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Response subset of the OpenAI-compatible `/v1/models`.
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetch models from one provider base URL. Returns raw ids (sorted, deduped).
/// Optional api_key is sent as Bearer. `base_url` should have no trailing slash.
pub async fn fetch_v1_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, FetchError> {
    let url = format!("{base_url}/v1/models");
    let req = client.get(&url);
    let req = match api_key {
        Some(k) => req.bearer_auth(k),
        None => req,
    };
    let resp = timeout(Duration::from_secs(10), req.send())
        .await
        .map_err(|_| FetchError::Timeout)?
        .map_err(|e| FetchError::Http(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Status(status.as_u16()));
    }

    let body: ModelsResponse = resp
        .json()
        .await
        .map_err(|e| FetchError::Json(e.to_string()))?;
    let mut ids: Vec<String> = body.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Errors shown in the log pane (PRD FE-3/§4 error display).
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("request timed out after 10s")]
    Timeout,
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("provider returned status {0}")]
    Status(u16),
    #[error("invalid JSON: {0}")]
    Json(String),
}

/// Fallback catalog for Anthropic-native endpoints (PRD FE-3.2).
pub const ANTHROPIC_NATIVE_MODELS: &[&str] = &[
    "claude-3-5-sonnet-20241022",
    "claude-3-opus-20241022",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
    "claude-2.1",
];

/// Shared catalog pipeline: static fallback ∪ live results for the configured
/// providers. Publish into the given Arc<tokio::sync::Mutex<HashSet>>.
pub async fn refresh_catalog(
    base_urls: Vec<String>,
    catalog: Arc<tokio::sync::RwLock<HashSet<String>>>,
    api_key_env: Option<String>,
) -> Vec<(String, Result<usize, FetchError>)> {
    let client = reqwest::Client::new();
    let api_key = api_key_env.and_then(|k| std::env::var(k).ok());
    let mut handles = Vec::new();
    for base in &base_urls {
        let base = base.clone();
        let client = client.clone();
        let key = api_key.clone();
        let catalog = catalog.clone();
        handles.push(tokio::spawn(async move {
            match fetch_one_with_base(&client, &base, key.as_deref()).await {
                Ok((b, ids)) => {
                    let n = ids.len();
                    catalog.write().await.extend(ids);
                    (b.clone(), Ok(n))
                }
                Err((b, e)) => (b, Err(e)),
            }
        }));
    }
    let mut results = Vec::new();
    for h in handles {
        match h.await {
            Ok(pair) => results.push(pair),
            Err(e) => results.push(("(spawn)".into(), Err(FetchError::Http(e.to_string())))),
        }
    }
    results
}

/// Fetch a single provider; on error the first tuple element is the base URL
/// (so callers can log or display precision-targeted errors).
pub async fn fetch_one_with_base(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<(String, Vec<String>), (String, FetchError)> {
    fetch_v1_models(client, base_url, api_key)
        .await
        .map(|v| (base_url.to_string(), v))
        .map_err(|e| (base_url.to_string(), e))
}
