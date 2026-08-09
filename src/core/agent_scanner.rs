//! Recursive scanner discovering `*.md` agent files in project + global scopes.
//!
//! Mirrors upstream OpenCode layering: global agents (from
//! `~/.config/opencode/agents/`) load first, project agents (from
//! `<root>/.opencode/agents/`) afterwards and shadow same-named globals.
//!
//! Project scanning accepts both layouts used by upstream:
//!   <root>/.opencode/agents/*.md
//!   <root>/.opencode/agent/*.md     (upstream glob covers singular too)
//!
//! Global scanning accepts only `<XDG_CONFIG_HOME>/opencode/agents/` (and its
//! singular alias). Falls back to `~/.config/opencode/agents/` when XDG is
//! unset, on every OS incl. Windows.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::config_resolver::global_config_dir;

/// A discovered agent file plus which scope supplied it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEntry {
    pub path: PathBuf,
    pub origin: AgentOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentOrigin {
    Project,
    Global,
}

/// Scan project scope only. Returns empty vec when dir missing.
pub fn scan_agents(root: &Path) -> io::Result<Vec<PathBuf>> {
    scan_one(&root.join(".opencode"), None)
}

/// Scan project + global, dedup by filename (project wins).
pub fn scan_agents_cascaded(root: &Path) -> io::Result<Vec<AgentEntry>> {
    let project = scan_one(&root.join(".opencode"), None)?
        .into_iter()
        .map(|p| AgentEntry {
            path: p,
            origin: AgentOrigin::Project,
        })
        .collect::<Vec<_>>();

    let global = match global_config_dir() {
        Some(dir) => scan_one(&dir, None)?,
        None => Vec::new(),
    }
    .into_iter()
    .map(|p| AgentEntry {
        path: p,
        origin: AgentOrigin::Global,
    })
    .collect::<Vec<_>>();

    let mut out = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    // Project first => names recorded; global entries with same name dropped.
    for e in project {
        if let Some(n) = e.path.file_name() {
            seen_names.insert(n.to_owned());
        }
        out.push(e);
    }
    for e in global {
        let name = match e.path.file_name() {
            Some(n) => n,
            None => continue,
        };
        if seen_names.contains(name) {
            continue; // shadowed by project
        }
        seen_names.insert(name.to_owned());
        out.push(e);
    }
    Ok(out)
}

/// Walk one scope directory. Accepts both `agents/` and `agent/` singular.
fn scan_one(scope_dir: &Path, _origin: Option<AgentOrigin>) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for name in ["agents", "agent"] {
        let dir = scope_dir.join(name);
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_support::ENV_LOCK;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ocoger-scan-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn scan_agents_empty_dir() {
        let d = temp_dir("empty");
        assert_eq!(scan_agents(&d).unwrap(), Vec::<PathBuf>::new());
    }

    #[test]
    fn project_only_returns_project() {
        let _g = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let project = temp_dir("proj");
        // Global sandbox forced to an EMPTY dir — isolates from real ~/.config.
        let global_sandbox = temp_dir("global");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global_sandbox);
        std::fs::create_dir_all(project.join(".opencode/agents")).unwrap();
        std::fs::write(
            project.join(".opencode/agents/a.md"),
            "---\nmodel: m\n---\n",
        )
        .unwrap();
        let out = scan_agents_cascaded(&project).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].origin, AgentOrigin::Project);
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }

    #[test]
    fn global_fallback_when_project_has_none() {
        let _g = ENV_LOCK.lock().unwrap();
        let project = temp_dir("only-global-proj");
        let global = temp_dir("only-global-xdg");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global);
        std::fs::create_dir_all(global.join("agents")).unwrap();
        std::fs::write(global.join("agents/g.md"), "---\nmodel: g\n---\n").unwrap();

        let out = scan_agents_cascaded(&project).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].origin, AgentOrigin::Global);
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }

    #[test]
    fn project_shadows_global_by_name() {
        let _g = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let project = temp_dir("shadow-proj");
        let global = temp_dir("shadow-xdg");
        std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", &global);
        std::fs::create_dir_all(project.join(".opencode/agents")).unwrap();
        std::fs::create_dir_all(global.join("agents")).unwrap();
        std::fs::write(project.join(".opencode/agents/dup.md"), "proj").unwrap();
        std::fs::write(global.join("agents/dup.md"), "glob").unwrap();

        let out = scan_agents_cascaded(&project).unwrap();
        let dup = out
            .iter()
            .find(|e| e.path.file_name().unwrap() == "dup.md")
            .unwrap();
        assert_eq!(dup.origin, AgentOrigin::Project);
        assert_eq!(out.len(), 1);
        std::env::remove_var("OCOGAR_GLOBAL_CONFIG_DIR");
    }
}
