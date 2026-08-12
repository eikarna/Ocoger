//! Commands (.opencode/commands/*.md) — templates with name + description frontmatter.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub name: String,
    pub description: String,
    /// Optional OpenCode command frontmatter fields, editable like subagents.
    pub agent: Option<String>,
    pub model: Option<String>,
    /// Source path; needed to update frontmatter without touching the body.
    pub path: std::path::PathBuf,
}

#[derive(Debug, Default)]
pub struct CommandFields {
    pub description: String,
    pub agent: Option<String>,
    pub model: Option<String>,
}

impl Command {
    /// Parse YAML frontmatter from a command file body.
    pub fn parse_from_frontmatter(raw_yaml: &str) -> Result<Self, ParseError> {
        let map: HashMap<String, String> =
            serde_yaml::from_str(raw_yaml).map_err(|e| ParseError::Deserialize(e.to_string()))?;

        let desc = map.get("description").cloned().unwrap_or_default();
        let name = map.get("name").cloned().unwrap_or_default();
        let agent = map.get("agent").cloned();
        let model = map.get("model").cloned();

        if name.is_empty() {
            return Err(ParseError::MissingField("name"));
        }
        if desc.is_empty() {
            return Err(ParseError::MissingField("description"));
        }

        Ok(Self {
            name,
            description: desc,
            agent,
            model,
            path: std::path::PathBuf::new(),
        })
    }

    /// Fields surfaced in the command form. `name` is optional because the
    /// filename is the OpenCode command name.
    pub fn frontmatter_fields(raw_yaml: &str) -> CommandFields {
        let map: HashMap<String, serde_yaml::Value> =
            serde_yaml::from_str(raw_yaml).unwrap_or_default();
        let string = |key: &str| {
            map.get(key)
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_string)
        };
        CommandFields {
            description: string("description").unwrap_or_default(),
            agent: string("agent"),
            model: string("model"),
        }
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

/// Scan project and global command dirs, project shadowing global by name.
/// OpenCode reads `command/` (singular) as well as the older `commands/`;
/// both are accepted here so either layout is picked up.
pub fn list_commands(project_root: &std::path::Path) -> Result<Vec<Command>, ScanError> {
    let mut by_name: std::collections::BTreeMap<String, Command> =
        std::collections::BTreeMap::new();

    // Global first, so project entries overwrite same-named globals.
    if let Some(global) = crate::core::config_resolver::global_config_dir() {
        for dir in [global.join("command"), global.join("commands")] {
            scan_dir_into(&dir, &mut by_name)?;
        }
    }
    let project = project_root.join(".opencode");
    for dir in [project.join("command"), project.join("commands")] {
        scan_dir_into(&dir, &mut by_name)?;
    }

    Ok(by_name.into_values().collect())
}

fn scan_dir_into(
    dir: &std::path::Path,
    out: &mut std::collections::BTreeMap<String, Command>,
) -> Result<(), ScanError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            // `name` is optional in OpenCode command files — the filename is
            // the command name. Only `description` comes from frontmatter.
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if stem.is_empty() {
                continue;
            }
            let fields = find_delimiters(&content)
                .map(|(y1, y2)| &content[y1..y2])
                .map(Command::frontmatter_fields)
                .unwrap_or_default();
            out.insert(
                stem.clone(),
                Command {
                    name: stem,
                    description: fields.description,
                    agent: fields.agent,
                    model: fields.model,
                    path,
                },
            );
        }
    }
    Ok(())
}

/// Replace or insert one scalar command frontmatter field, preserving the
/// Markdown body and unrelated YAML. Only fields shown in the command form are
/// accepted, so UI input cannot inject arbitrary YAML keys.
pub fn set_frontmatter_field(
    path: &std::path::Path,
    key: &str,
    value: &str,
) -> Result<(), ScanError> {
    if !matches!(key, "description" | "agent" | "model") {
        return Err(ScanError::InvalidField(key.to_string()));
    }
    let raw = std::fs::read_to_string(path)?;
    let next = if let Some((start, end)) = find_delimiters(&raw) {
        let yaml = &raw[start..end];
        // Avoid reserializing the YAML or touching the Markdown body.
        let mut lines: Vec<String> = yaml.lines().map(str::to_string).collect();
        // Let serde_yaml quote colons, quotes, and other scalar edge cases;
        // only take the one emitted line, retaining every unrelated YAML line.
        let encoded = serde_yaml::to_string(&serde_yaml::Value::String(value.to_string()))
            .unwrap_or_else(|_| format!("{value:?}\n"));
        let encoded = encoded.trim();
        if let Some(line) = lines.iter_mut().find(|l| {
            l.trim_start()
                .strip_prefix(key)
                .is_some_and(|tail| tail.trim_start().starts_with(':'))
        }) {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            *line = format!("{indent}{key}: {encoded}");
        } else {
            lines.push(format!("{key}: {encoded}"));
        }
        format!("{}{}{}", &raw[..start], lines.join("\n"), &raw[end..])
    } else {
        format!("---\n{key}: {value}\n---\n{raw}")
    };
    crate::core::fs_util::atomic_write(path, &next)?;
    Ok(())
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
    #[error("unsupported command frontmatter field: {0}")]
    InvalidField(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Point the global-config lookup at an empty temp dir so the developer's
    /// real `~/.config/opencode/command` can't leak into these assertions.
    fn isolated_root(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-cmd-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("global-empty")).unwrap();
        unsafe {
            std::env::set_var("OCOGAR_GLOBAL_CONFIG_DIR", dir.join("global-empty"));
        }
        dir
    }

    #[test]
    fn scan_parses_valid_command_frontmatter() {
        let _guard = crate::core::test_support::ENV_LOCK.lock().unwrap();
        let dir = isolated_root("parse");
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
        assert!(out[0].agent.is_none());
        assert!(out[0].model.is_none());
    }

    /// OpenCode command files often carry only `description` (or no frontmatter
    /// at all) — the filename is the command name. Those must still be listed.
    #[test]
    fn scan_accepts_files_without_name_field() {
        let _guard = crate::core::test_support::ENV_LOCK.lock().unwrap();
        let dir = isolated_root("noname");
        // Singular `command/` is the layout OpenCode itself uses.
        let cdir = dir.join(".opencode").join("command");
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::write(
            cdir.join("ship.md"),
            "---\ndescription: cut a release\n---\nrun the release steps\n",
        )
        .unwrap();
        std::fs::write(cdir.join("bare.md"), "no frontmatter at all\n").unwrap();
        let out = list_commands(&dir).unwrap();
        assert_eq!(out.len(), 2, "both files listed");
        assert_eq!(out[0].name, "bare", "filename is the command name");
        assert_eq!(out[0].description, "", "missing description is empty");
        assert_eq!(out[1].name, "ship");
        assert_eq!(out[1].description, "cut a release");
    }

    #[test]
    fn set_description_preserves_body_and_replaces_only_frontmatter_field() {
        let _guard = crate::core::test_support::ENV_LOCK.lock().unwrap();
        let dir = isolated_root("edit");
        let path = dir.join("command.md");
        std::fs::write(
            &path,
            "---\ndescription: old words\nagent: build\nmodel: 9router/deepseek-v4\n---\n# Body\nkeep this body\n",
        )
        .unwrap();
        set_frontmatter_field(&path, "description", "new words").unwrap();
        set_frontmatter_field(&path, "model", "9router/kimi-k3").unwrap();
        let out = std::fs::read_to_string(path).unwrap();
        assert!(out.contains("description: new words"));
        assert!(out.contains("agent: build"), "sibling frontmatter retained");
        assert!(
            out.contains("model: 9router/kimi-k3"),
            "target scalar replaced"
        );
        assert!(
            out.ends_with("# Body\nkeep this body\n"),
            "body byte sequence retained"
        );
    }

    #[test]
    fn scan_missing_dir_is_empty_not_error() {
        let _guard = crate::core::test_support::ENV_LOCK.lock().unwrap();
        let dir = isolated_root("missing");
        let out = list_commands(&dir.join("no-such-root")).unwrap();
        assert!(out.is_empty());
    }
}
