//! Recursive scanner discovering `.opencode/agents/*.md` agent files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Find all `.md` files under `<root>/.opencode/agents/` (non-recursive for MVP).
pub fn scan_agents(root: &Path) -> io::Result<Vec<PathBuf>> {
    let agents_dir = root.join(".opencode").join("agents");
    let mut out = Vec::new();
    if !agents_dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
