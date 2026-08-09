//! Manual verification harness: pulls every `.md` file under the real
//! `%USERPROFILE%/.config/opencode/agents/` (or `$XDG_CONFIG_HOME/opencode/agents`)
//! and asserts each parses without error.
//!
//! Run only on your workstation — CI never has this dir, so this stays
//! `#[ignore]`-gated.

use ocoger::core::agent_parser::parse_agent;

#[test]
#[ignore = "inspects the developer's real agents directory; run manually with --ignored"]
fn parses_every_global_agent_file_on_disk() {
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(std::path::PathBuf::from)
    } else {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }
    .expect("no home dir");
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let dir = xdg.join("opencode").join("agents");
    assert!(
        dir.is_dir(),
        "expected global agents dir at {}",
        dir.display()
    );

    let mut read = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        read += 1;
        let content = std::fs::read_to_string(&path).unwrap();
        parse_agent(&content).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    assert!(read > 0, "expected at least one global agent in {dir:?}");
}
