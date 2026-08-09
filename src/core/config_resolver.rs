//! OpenCode config resolution: project-first, global-fallback, per-key merge.
//!
//! Mirrors the official `sst/opencode` cascade (minus remote / managed paths):
//!   1. Project config: walked up from `project_root` to the FS root, looking for
//!      `opencode.jsonc`, then `opencode.json`, then `config.json`.
//!   2. Global config dir: `$XDG_CONFIG_HOME/opencode/` else `~/.config/opencode/`
//!      (same on all OSes incl. Windows; `XDG_CONFIG_HOME` overrides).
//!   3. Inside the global dir, the same filename candidates as the project.
//!
//! Env overrides honored on merge: `OPENCODE_CONFIG` (explicit file, wins over
//! project+global), `OPENCODE_DISABLE_PROJECT_CONFIG` (skips project entirely).
//!
//! Merge: deep per-key. Project value wins on conflict. Objects merge
//! recursively; arrays + scalars are taken whole from the higher-precedence
//! source.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::jsonc_config::{ConfigError, JsoncConfig};

/// The three filename candidates, in descending precedence order, matching
/// the upstream resolver (`packages/opencode/src/config/config.ts`).
pub const CONFIG_FILE_CANDIDATES: &[&str] = &["opencode.jsonc", "opencode.json", "config.json"];

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Config(#[from] ConfigError),
}

/// Debug/provenance record for one merged config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Path the eventual in-memory view was loaded from / written to.
    pub primary_path: Option<PathBuf>,
    /// All paths that contributed to the merge (project first, global later).
    pub sources: Vec<PathBuf>,
    /// Whether the project-level config was consulted. False when
    /// `OPENCODE_DISABLE_PROJECT_CONFIG` is set.
    pub project_enabled: bool,
}

pub struct ResolvedConfig {
    /// Merged view. Some(k) means at least one source contributed at least
    /// one key. None means *no* config source has been seen at all.
    pub merged_value: Option<Value>,
    pub resolution: Resolution,
}

/// Locate the global OpenCode config directory.
/// Honors `XDG_CONFIG_HOME`. Falls back to `~/.config/opencode` via the
/// platform's notion of home. On Windows the home dir is `%USERPROFILE%`.
///
/// `OCOGAR_GLOBAL_CONFIG_DIR` is an **ocoger-internal override** used by the
/// test suite (and power users) to point the global lookup at a controlled
/// directory. It never ships into upstream opencode resolution paths.
pub fn global_config_dir() -> Option<PathBuf> {
    if let Some(x) = env::var_os("OCOGAR_GLOBAL_CONFIG_DIR") {
        return Some(PathBuf::from(x));
    }
    if let Some(x) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(x).join("opencode"));
    }
    home_dir().map(|h| h.join(".config").join("opencode"))
}

/// The cross-platform "home dir" primitive used by open-code style tools.
fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

/// Walk a directory upward looking for a config file matching
/// `CONFIG_FILE_CANDIDATES`. First hit wins.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start.to_path_buf());
    while let Some(d) = cur {
        for name in CONFIG_FILE_CANDIDATES {
            let p = d.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        // `.opencode/opencode.jsonc` layout (`.opencode/` wins over sibling
        // `opencode.json`).
        for name in CONFIG_FILE_CANDIDATES {
            let p = d.join(".opencode").join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        cur = d.parent().map(Path::to_path_buf);
    }
    None
}

pub fn find_global_config() -> Option<PathBuf> {
    let dir = global_config_dir()?;
    for name in CONFIG_FILE_CANDIDATES {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Resolve the effective config: env-override → project-walkup → global.
/// Returns the merged JSONC view plus provenance; `None` means no source
/// published anything (that's fine: App falls back to defaults).
pub fn resolve(project_root: &Path) -> Result<ResolvedConfig, ResolveError> {
    let project_enabled = env::var_os("OPENCODE_DISABLE_PROJECT_CONFIG").is_none();

    // Collect raw file contents in priority order.
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(p) = env::var_os("OPENCODE_CONFIG").map(PathBuf::from) {
        if p.is_file() {
            paths.push(p);
        }
    }
    if project_enabled {
        if let Some(p) = find_project_config(project_root) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
    }
    if let Some(p) = find_global_config() {
        if !paths.contains(&p) {
            paths.push(p);
        }
    }

    if paths.is_empty() {
        return Ok(ResolvedConfig {
            merged_value: None,
            resolution: Resolution {
                primary_path: None,
                sources: Vec::new(),
                project_enabled,
            },
        });
    }

    let mut merged_acc: Option<Value> = None;
    for path in &paths {
        let raw = std::fs::read_to_string(path)?;
        let value = jsonc_parser::parse_to_serde_value::<Value>(
            &raw,
            &jsonc_parser::ParseOptions::default(),
        )
        .map_err(|e| ResolveError::Config(ConfigError::Parse(e.to_string())))?;
        merged_acc = match (merged_acc, value) {
            (None, v) => Some(v),
            (Some(acc), v) => Some(merge_value(&acc, &v)),
        };
    }

    Ok(ResolvedConfig {
        merged_value: merged_acc,
        resolution: Resolution {
            primary_path: paths.first().cloned(),
            sources: paths,
            project_enabled,
        },
    })
}

/// Deep merge. `higher` wins on conflict. Objects merge recursively;
/// every other type is taken whole from `higher` when present.
pub fn merge_value(higher: &Value, lower: &Value) -> Value {
    match (higher, lower) {
        (Value::Object(h), Value::Object(l)) => {
            let mut out: Map<String, Value> = Map::with_capacity(h.len() + l.len());
            for (k, v) in l.iter() {
                out.insert(k.clone(), v.clone());
            }
            for (k, v) in h.iter() {
                let merged = match (out.get(k), v) {
                    (Some(l_v), h_v) => merge_value(h_v, l_v),
                    (None, h_v) => h_v.clone(),
                };
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        (h, _) => h.clone(),
    }
}

/// Drop-in replacement for `JsoncConfig::ensure_loaded` that consults the
/// cascade instead of just `<project>/opencode.jsonc`. Bootstraps the first
/// project-level config file when missing.
///
/// Behavior notes:
/// - `existing = project file path` if there is one.
/// - If not, the global config may be consulted for *read* values only;
///   writes still land in `<project>/opencode.jsonc` (project-local breaking
///   the user's primary config surface).
pub fn ensure_loaded(project_root: &Path) -> Result<JsoncConfig, ResolveError> {
    let resolved = resolve(project_root)?;
    let primary = resolved
        .resolution
        .primary_path
        .unwrap_or_else(|| project_root.join("opencode.jsonc"));
    if primary.is_file() {
        // Hand off to existing reader so we don't re-parse raw bytes ourselves.
        // (Cheap: reader just re-reads + parses; already-CST-safe.)
        if let Some(cfg) = JsoncConfig::load_at_path(&primary)? {
            return Ok(cfg);
        }
    }
    JsoncConfig::ensure_loaded(project_root).map_err(ResolveError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    // Tests mutate process env (XDG/OCOGAR/OPENCODE vars). Serialize.Test-safe Mutex shared only within this module.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-cfgres-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn project_config_beats_sibling_dot_opencode() {
        let dir = temp_root("walkup");
        // Root has top-level config; child has its own .opencode
        fs::write(dir.join("opencode.jsonc"), r#"{ "model": "root-model" }"#).unwrap();
        let child = dir.join("sub/dir");
        fs::create_dir_all(&child).unwrap();
        fs::create_dir_all(child.join(".opencode")).unwrap();
        fs::write(
            child.join(".opencode/opencode.jsonc"),
            r#"{ "model": "child-model" }"#,
        )
        .unwrap();

        // Walker from `child` should find the local .opencode config first.
        let found = find_project_config(&child).expect("some config found");
        assert!(
            found.ends_with(".opencode\\opencode.jsonc")
                || found.ends_with(".opencode/opencode.jsonc"),
            "expected .opencode precedence, got: {}",
            found.display()
        );
    }

    #[test]
    fn walkup_skips_dirs_without_config() {
        let dir = temp_root("walkup-skip");
        let deep = dir.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(dir.join("opencode.jsonc"), r#"{ "x": 1 }"#).unwrap();
        let found = find_project_config(&deep).expect("some config");
        assert!(found.ends_with("opencode.jsonc"));
        assert_eq!(found.parent().unwrap().to_path_buf(), dir);
    }

    #[test]
    fn deep_merge_prefers_higher_priority() {
        let lower = serde_json::json!({
            "model": "anthropic/old",
            "theme": "light",
            "provider": { "openai": { "base_url": "api", "api_key": "k1" } }
        });
        let higher = serde_json::json!({
            "model": "anthropic/new",
            "provider": { "openai": { "api_key": "k2" }, "anthropic": {} }
        });

        let got = merge_value(&higher, &lower);

        assert_eq!(got["model"], "anthropic/new", "higher wins on overlap");
        assert_eq!(
            got["theme"], "light",
            "non-overlapping key copied from lower"
        );
        let p = &got["provider"];
        assert_eq!(
            p["openai"]["base_url"], "api",
            "nested non-overlap preserved"
        );
        assert_eq!(p["openai"]["api_key"], "k2", "nested overlap: higher wins");
        assert!(p.get("anthropic").is_some(), "added key present");
    }

    #[test]
    fn merge_scalars_and_arrays_from_higher() {
        let lower = serde_json::json!({"arr": [1, 2, 3], "n": 7});
        let higher = serde_json::json!({"arr": ["a"], "n": 9});
        let got = merge_value(&higher, &lower);
        assert_eq!(got["arr"], serde_json::json!(["a"]));
        assert_eq!(got["n"], 9);
    }

    #[test]
    fn resolve_falls_back_to_project_root_when_nothing_exists() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_root("empty");
        let global_sandbox = temp_root("empty-global");
        // Isolate from the developer's real ~/.config/opencode so the
        // assertion is deterministic.
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global_sandbox);
        let out = resolve(&dir).unwrap();
        assert!(out.merged_value.is_none());
        assert_eq!(out.resolution.primary_path, None);
        assert!(out.resolution.project_enabled);
        // ensure_loaded bootstraps a valid default in memory; save() then
        // persists the project-root JSONC.
        let cfg = ensure_loaded(&dir).unwrap();
        cfg.save().unwrap();
        let expected = dir.join("opencode.jsonc");
        assert!(expected.is_file(), "bootstrapped default config");
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }

    #[test]
    fn project_overrides_global_per_key() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_root("cascade");
        let global_sandbox = temp_root("cascade-global");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global_sandbox);
        // Project file sets model only; global sets theme+model. Project wins model.
        fs::write(dir.join("opencode.jsonc"), r#"{"model":"proj-model"}"#).unwrap();
        fs::write(
            global_sandbox.join("opencode.jsonc"),
            r#"{"model":"global-model","theme":"dark"}"#,
        )
        .unwrap();

        let out = resolve(&dir).unwrap();
        let v = out.merged_value.expect("merged");
        assert_eq!(v["model"], "proj-model", "project wins on conflict");
        assert_eq!(v["theme"], "dark", "global-only key survives");
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }

    #[test]
    fn opencode_config_env_wins_over_project_and_global() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_root("env-override");
        let global_sandbox = temp_root("env-global");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global_sandbox);
        fs::write(dir.join("opencode.jsonc"), r#"{"model":"proj"}"#).unwrap();
        fs::write(global_sandbox.join("opencode.jsonc"), r#"{"model":"glob"}"#).unwrap();
        let explicit = temp_root("env-explicit").join("explicit.jsonc");
        fs::write(&explicit, r#"{"model":"env-model"}"#).unwrap();

        std::env::set_var("OPENCODE_CONFIG", &explicit);
        let out = resolve(&dir).unwrap();
        let v = out.merged_value.expect("merged");
        assert_eq!(v["model"], "env-model");
        // Order sanity: env override sits at the top of sources.
        assert_eq!(out.resolution.sources.first().unwrap(), &explicit);
        std::env::remove_var("OPENCODE_CONFIG");
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }

    #[test]
    fn disable_project_config_skips_local_walk() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_root("disable");
        let global_sandbox = temp_root("disable-global");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global_sandbox);
        fs::write(dir.join("opencode.jsonc"), r#"{"model":"proj"}"#).unwrap();
        fs::write(global_sandbox.join("opencode.jsonc"), r#"{"model":"glob"}"#).unwrap();

        std::env::set_var("OPENCODE_DISABLE_PROJECT_CONFIG", "1");
        let out = resolve(&dir).unwrap();
        let v = out.merged_value.expect("merged");
        assert_eq!(v["model"], "glob", "project entry must be ignored");
        assert!(!out.resolution.project_enabled);
        std::env::remove_var("OPENCODE_DISABLE_PROJECT_CONFIG");
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }
}
