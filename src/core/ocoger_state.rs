//! Ocoger-owned per-project state.
//!
//! This is intentionally isolated under `.ocoger/` so Ocoger never creates or
//! edits an OpenCode `opencode.json(c)` merely to store its own UI preference.

use std::path::Path;

const STATE_FILE: &str = "state.toml";

/// Best-effort read of `.ocoger/state.toml`'s `theme` value. Invalid or absent
/// state is equivalent to default theme; UI preferences must never block boot.
pub fn load_theme_name(project: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(project.join(".ocoger").join(STATE_FILE)).ok()?;
    let doc: toml::Value = raw.parse().ok()?;
    doc.get("theme")?.as_str().map(str::to_owned)
}

/// Persist only the Ocoger theme. Kept tiny and atomic via the existing helper.
pub fn save_theme_name(project: &Path, name: &str) -> std::io::Result<()> {
    let dir = project.join(".ocoger");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(STATE_FILE);
    crate::core::fs_util::atomic_write(&path, &format!("theme = {name:?}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_theme_does_not_touch_opencode_config() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        save_theme_name(&dir, "tokyonight").unwrap();
        assert_eq!(load_theme_name(&dir).as_deref(), Some("tokyonight"));
        assert!(dir.join(".ocoger").join(STATE_FILE).is_file());
        assert!(
            !dir.join("opencode.jsonc").exists(),
            "Ocoger state must never create an OpenCode config"
        );
    }
}
