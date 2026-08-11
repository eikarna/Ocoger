//! Commands (.opencode/commands/*.md) — templates with name + description frontmatter.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub name: String,
    pub description: String,
}

impl Command {
    /// Parse YAML frontmatter from a command file body.
    pub fn parse_from_frontmatter(raw_yaml: &str) -> Result<Self, ParseError> {
        let map: HashMap<String, String> =
            serde_yaml::from_str(raw_yaml).map_err(|e| ParseError::Deserialize(e.to_string()))?;

        let desc = map.get("description").cloned().unwrap_or_default();
        let name = map.get("name").cloned().unwrap_or_default();

        if name.is_empty() {
            return Err(ParseError::MissingField("name"));
        }
        if desc.is_empty() {
            return Err(ParseError::MissingField("description"));
        }

        Ok(Self {
            name,
            description: desc,
        })
    }

    /// Serialize command → YAML frontmatter.
    pub fn serialize(&self) -> String {
        format!(
            r#"---
name: {}
description: {}
---"#,
            self.name, self.description
        )
    }
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid YAML: {0}")]
    Deserialize(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Scan `.opencode/commands/*.md` and collect all commands.
pub fn list_commands(project_root: &std::path::Path) -> Result<Vec<Command>, ScanError> {
    let cmd_dir = project_root.join(".opencode").join("commands");
    if !cmd_dir.exists() {
        return Ok(Vec::new());
    }

    let mut commands = Vec::new();
    for entry in std::fs::read_dir(cmd_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(&path).ok().unwrap_or_default();
            if let Some((y1, y2)) = find_delimiters(&content) {
                let raw_yaml = &content[y1..y2];
                match Command::parse_from_frontmatter(raw_yaml) {
                    Ok(cmd) => commands.push(cmd),
                    Err(_) => {}
                }
            }
        }
    }
    commands.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(commands)
}

fn find_delimiters(content: &str) -> Option<(usize, usize)> {
    let start = content.find("---\n")? + 4;
    let rest = &content[start..];
    let end = rest.find("\n---")?;
    Some((start, start + end))
}

#[derive(Error, Debug)]
pub enum ScanError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_parses_valid_command_frontmatter() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-cmd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let cdir = dir.join(".opencode").join("commands");
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::write(
            cdir.join("fixup.md"),
            "---\nname: fixup\ndescription: amend last commit\n---\nbody text\n",
        )
        .unwrap();
        let out = list_commands(&dir).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "fixup");
        assert_eq!(out[0].description, "amend last commit");
    }

    #[test]
    fn scan_missing_dir_is_empty_not_error() {
        let out = list_commands(std::path::Path::new("no-such-root")).unwrap();
        assert!(out.is_empty());
    }
}
