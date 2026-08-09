//! Test-only helpers shared across core test modules.
//!
//! Several tests mutate process env (`XDG_CONFIG_HOME`, `OPENCODE_*`,
//! `OCOGAR_*`). Running them concurrently is undefined behavior on Windows
//! (`std::env::set_var` is process-wide). A single cross-module Mutex ensures
//! no two of them race.

#[cfg(test)]
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
