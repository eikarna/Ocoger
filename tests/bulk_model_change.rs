//! Bulk model-change integration test (TODO §6 / Phase 6).
//! Exercises the full path that `m`/`p` UI stages:
//!   scan_agents -> load_agent -> update_models -> AgentFile::save (atomic)
//! against a temp dir of synthetic `.opencode/agents/*.md` files.
//!
//! Asserts:
//!   1. All N agents get the new model value persisted.
//!   2. YAML comments and key order survive (byte-surgical splice, not serde).
//!   3. No stray `.tmp` files remain after atomic rename.

use ocoger::core::agent_parser::load_agent;
use ocoger::core::agent_scanner::scan_agents;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Distinct model for agent i so the test would catch cross-contamination.
fn fixture_for(i: usize) -> String {
    format!(
        r#"---
# agent #{i} header comment
description: Synthetic agent {i}
model: anthropic/claude-3-7  # primary
temperature: 0.2
reasoning_effort: high
top_p: 0.9  # trailing anchor
---
# Agent {i} Body

Some paragraph that stays verbatim.

```text
--- delimiter inside body
```
"#
    )
}

#[test]
fn bulk_model_change_across_20_agents_preserves_fidelity() {
    const N: usize = 20;
    let tmp = TempDir::new().expect("tempdir");
    let agents_dir = tmp.path().join(".opencode").join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Seed N agents with distinct fixture content.
    for i in 0..N {
        let path = agents_dir.join(format!("agent-{i:02}.md"));
        fs::write(&path, fixture_for(i)).expect("seed write");
    }

    // 1. Scan must see exactly N files, sorted.
    let paths = scan_agents(tmp.path()).expect("scan");
    assert_eq!(paths.len(), N, "scanner must discover all agents");
    let names: Vec<String> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "scanner must return sorted");

    // 2. Load-all -> set_model -> save-all (mirrors batch apply in UI).
    const NEW: &str = "openai/gpt-5";
    for path in &paths {
        let mut agent = load_agent(path).expect("load");
        assert!(
            agent.frontmatter.model.starts_with("anthropic/claude-3-7"),
            "precondition: distinct model before edit, got {}",
            agent.frontmatter.model
        );
        agent.update_models(&[("model".into(), NEW.into())]);
        assert!(agent.is_dirty, "must mark dirty after update");
        agent.save().expect("atomic save");
        assert!(!agent.is_dirty, "save must clear dirty");
    }

    // 3. Re-read every file from disk and verify byte-level invariants.
    for (i, path) in paths.iter().enumerate() {
        let disk = fs::read_to_string(path).expect("read back");
        // Model flipped.
        assert!(
            disk.contains(&format!("model: {NEW}  # primary")),
            "agent {i}: model must be replaced in-place keeping the comment"
        );
        assert!(
            !disk.contains("anthropic/claude-3-7"),
            "agent {i}: old model must be gone"
        );
        // Headers specific to agent i must still be present (no cross-write).
        assert!(disk.contains(&format!("Synthetic agent {i}")));
        assert!(disk.contains(&format!("agent #{i} header comment")));
        // All other per-agent YAML anchors survive.
        assert!(disk.contains("top_p: 0.9  # trailing anchor"));
        assert!(disk.contains("reasoning_effort: high"));
        // Body verbatim incl. interior --- delimiter.
        assert!(disk.contains(&format!("# Agent {i} Body")));
        assert!(disk.contains("--- delimiter inside body"));
    }

    // 4. No stray .tmp leaves from atomic write.
    let leftovers: Vec<_> = fs::read_dir(&agents_dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .filter(|n| n.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover tmp files: {leftovers:?}");

    // 5. Sanity: exactly N+0 entries on disk (no fixture spill).
    let count = fs::read_dir(&agents_dir).unwrap().count();
    assert_eq!(count, N, "directory must contain exactly the N agent files");

    // TempDir RAII drop cleans the sandbox.
    drop(tmp);
}

/// Scanner on an empty root returns empty vec, not error (regression).
#[test]
fn scan_agents_on_empty_root_returns_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let paths = scan_agents(tmp.path()).expect("scan empty");
    assert!(paths.is_empty());
}

/// Missing `.opencode/agents/` directory is a normal "empty" case too.
#[test]
fn scan_agents_missing_dir_returns_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("does-not-exist");
    assert!(!Path::new(&root).exists());
    let paths = scan_agents(&root).expect("scan missing");
    assert!(paths.is_empty());
}
