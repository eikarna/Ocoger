//! Pre-save diff computation: unified line diffs between an on-disk file and
//! a staged mirror, with zero additional memory stored per agent.

use similar::{ChangeTag, TextDiff};

pub fn unified_by_line(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let (sign, value) = match change.tag() {
            ChangeTag::Equal => (" ", change),
            ChangeTag::Delete => ("-", change),
            ChangeTag::Insert => ("+", change),
        };
        out.push_str(sign);
        out.push_str(&value.to_string());
        if !value.to_string().ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

pub struct StagingDiff {
    pub file_name: String,
    pub diff_text: String,
}

/// Compute per-agent diffs between the real files on disk (original) and the
/// current staged content (post-edit). Uses atomic write for the staging
/// mirror to avoid partial writes.
pub fn agent_diffs(
    agents: &[crate::core::agent_parser::AgentFile],
    project_root: &std::path::Path,
) -> Vec<StagingDiff> {
    let staging_dir = project_root.join(".ocoger").join("staging");
    std::fs::create_dir_all(&staging_dir).ok();
    let mut out = Vec::new();
    for a in agents.iter().filter(|a| a.is_dirty || a.is_selected) {
        let name = a
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| a.path.display().to_string());
        let staging_path = staging_dir.join(format!("{}.md", name.trim_end_matches(".md")));
        let original = std::fs::read_to_string(&a.path).unwrap_or_default();
        let staged = crate::core::agent_parser::ParsedAgent {
            frontmatter: a.frontmatter.clone(),
            raw_yaml: a.raw_yaml.clone(),
            raw_body: a.raw_body.clone(),
        };
        let serialized = crate::core::agent_parser::serialize_agent(&staged);
        crate::core::fs_util::atomic_write(&staging_path, &serialized).ok();
        if original != serialized {
            out.push(StagingDiff {
                file_name: name,
                diff_text: unified_by_line(&original, &serialized),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_parser::{AgentFile, AgentFrontmatter};

    fn a(name: &str, model: &str) -> AgentFile {
        AgentFile {
            path: std::path::PathBuf::from(name),
            frontmatter: AgentFrontmatter {
                model: model.into(),
                temperature: None,
                top_k: None,
                top_p: None,
                reasoning_effort: None,
            },
            raw_body: String::new(),
            is_selected: false,
            is_dirty: true,
            raw_yaml: format!("model: {model}"),
            origin: crate::core::agent_parser::AgentOrigin::Project,
        }
    }

    #[test]
    fn unified_by_line_basic() {
        let diff = unified_by_line("a1\nb1\nc1\n", "a1\nb2\nc1\n");
        assert!(diff.contains("-b1"));
        assert!(diff.contains("+b2"));
        assert!(diff.split('\n').any(|l| l.starts_with('-')));
        assert!(diff.split('\n').any(|l| l.starts_with('+')));
    }

    #[test]
    fn agent_diffs_finds_model_change() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-diff-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("coder.md");
        std::fs::write(&path, "---\nmodel: old-model\n---\nbody\n").unwrap();

        let mut agent = crate::core::agent_parser::load_agent(&path).unwrap();
        agent.update_models(&[("model".into(), "new-model".into())]);
        // don't save — diff is pre-save
        let diffs = agent_diffs(&[agent], &dir);
        assert_eq!(diffs.len(), 1);
        let d = &diffs[0];
        assert_eq!(d.file_name, "coder.md");
        assert!(d.diff_text.contains("-model: old-model"));
        assert!(d.diff_text.contains("+model: new-model"));
    }
}
