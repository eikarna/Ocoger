//! Atomic file persistence (ARCH §4.1).
//!
//! Write goes to a `<name>.tmp` sibling first, then `rename`s over the target
//! so a crash mid-write never leaves a partially-written config.

use std::fs;
use std::io;
use std::path::Path;

pub fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let tmp = path.with_file_name(format!("{}.tmp", file_name.to_string_lossy()));

    fs::write(&tmp, content)?;
    // fs::rename overwrites an existing destination on both Unix and Windows.
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ocoger-test-{tag}-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn creates_new_file_and_leaves_no_tmp() {
        let path = temp_path("create");
        atomic_write(&path, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        let tmp = path.with_file_name(format!(
            "{}.tmp",
            path.file_name().unwrap().to_string_lossy()
        ));
        assert!(!tmp.exists(), "tmp file must be consumed by rename");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn overwrites_existing_file() {
        let path = temp_path("overwrite");
        fs::write(&path, "old").unwrap();
        atomic_write(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        fs::remove_file(&path).ok();
    }
}
