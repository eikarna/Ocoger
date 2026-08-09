//! Lifecycle supervisor for the `opencode` child process.
//!
//! Windows-aware design (ARCH §2.4, revised for this platform): no SIGTERM —
//! `Ctrl+S` triggers explicit `kill()` which is `TerminateProcess` on Windows
//! (the "graceful SIGTERM -> 3s timeout -> SIGKILL" semantics don't exist
//! here; `kill()` IS the graceful + forced path combined, then `wait()` with
//! timeout proves exit).
//!
//! stdout/stderr are piped and line-read into a channel via task, so the TUI
//! log pane can display tail lines without polling.

use anyhow::{anyhow, Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Minimal cross-platform "which" (PATH-aware):
/// tests candidates in order. We don't rely on `which`/`where` crates.
pub fn find_executable(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<&str> = if cfg!(windows) {
        vec![".cmd", ".exe", ".bat", ""]
    } else {
        vec![""]
    };
    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let p = dir.join(format!("{name}{ext}"));
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    None
}

const KILL_TIMEOUT_MS: u64 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Idle,
    Running,
    Stopped,
    Restarting,
}

pub struct ProcessManager {
    child: Option<Child>,
    pub state: ProcState,
    /// Receiver for output lines tagged with their stream name.
    pub output_rx: mpsc::UnboundedReceiver<(String, String)>,
    tx: mpsc::UnboundedSender<(String, String)>,
    pub pid: Option<u32>,
}

impl ProcessManager {
    pub fn new() -> Self {
        let (tx, output_rx) = mpsc::unbounded_channel();
        Self {
            child: None,
            state: ProcState::Idle,
            output_rx,
            tx,
            pid: None,
        }
    }

    /// Spawn `opencode` (or any cmd) in the given working dir. Pipes are wired
    /// to the output channel. No-op if already running.
    pub async fn spawn(&mut self, cwd: &std::path::Path) -> Result<u32> {
        if self.child.is_some() {
            return self.pid.ok_or_else(|| anyhow!("process already running"));
        }
        let cmd_name = find_executable("opencode").unwrap_or_else(|| "opencode".to_string());
        let mut cmd = Command::new(&cmd_name);
        cmd.current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut child = cmd.spawn().with_context(|| "spawn opencode failed")?;
        let pid = child.id().ok_or_else(|| anyhow!("spawn produced no pid"))?;

        let stdout = child.stdout.take().context("stdout not piped")?;
        let stderr = child.stderr.take().context("stderr not piped")?;
        let out_tx = self.tx.clone();
        let err_tx = self.tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = out_tx.send(("stdout".into(), line));
            }
        });
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = err_tx.send(("stderr".into(), line));
            }
        });

        self.child = Some(child);
        self.pid = Some(pid);
        self.state = ProcState::Running;
        Ok(pid)
    }

    /// Terminate the running process. Waits up to 3s for clean exit.
    pub async fn kill(&mut self) -> Result<u32> {
        let Some(mut child) = self.child.take() else {
            self.state = ProcState::Idle;
            return Ok(0);
        };
        let pid = self.pid.take().unwrap_or(0);
        child.kill().await.with_context(|| "kill failed")?;
        let _ = timeout(Duration::from_millis(KILL_TIMEOUT_MS), child.wait())
            .await
            .map_err(|_| anyhow!("post-kill wait timed out"))?;
        self.state = ProcState::Stopped;
        Ok(pid)
    }

    /// Restart: kill if running, then spawn. Returns the new pid.
    pub async fn restart(&mut self, cwd: &std::path::Path) -> Result<u32> {
        self.state = ProcState::Restarting;
        if self.child.is_some() {
            self.kill().await?;
        }
        self.spawn(cwd).await
    }

    /// Drain buffered output lines (non-blocking). The TUI calls this every
    /// frame and pushes lines into its App log.
    pub fn drain_output(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        while let Ok(line) = self.output_rx.try_recv() {
            out.push(line);
        }
        out
    }

    /// Drop-kill on Drop so a stray opencode.exe doesn't linger when ocoger
    /// exits.
    pub fn shutdown_sync(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        self.state = ProcState::Stopped;
        self.pid = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::process::Command;

    // Spawn a trivial short-lived child through the manager — we're testing
    // the manager state machine, not the child itself. `cmd` on Windows,
    // `sh` elsewhere (musl container images still ship /bin/sh via sh-symlink
    // on alpine; if not, fall back to `true`).
    async fn spawn_cmd(manager: &mut ProcessManager) -> u32 {
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.args(["/c", "echo", "test"]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", "echo test"]);
            c
        };
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();
        manager.child = Some(child);
        manager.pid = Some(pid);
        manager.state = ProcState::Running;
        pid
    }

    #[tokio::test]
    async fn kill_clears_state_and_returns_pid() {
        let mut m = ProcessManager::new();
        let pid = spawn_cmd(&mut m).await;
        // give child a moment to exit naturally to avoid zombie pollution
        tokio::time::sleep(Duration::from_millis(50)).await;
        let killed = m.kill().await.unwrap();
        assert_eq!(killed, pid);
        assert_eq!(m.pid, None);
        assert_eq!(m.state, ProcState::Stopped);
        assert!(m.child.is_none());
    }

    #[tokio::test]
    async fn double_kill_is_noop_not_error() {
        let mut m = ProcessManager::new();
        spawn_cmd(&mut m).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        m.kill().await.unwrap();
        let second = m.kill().await.unwrap();
        assert_eq!(second, 0);
        assert_eq!(m.state, ProcState::Idle);
    }

    #[tokio::test]
    async fn shutdown_sync_kills_without_await() {
        let mut m = ProcessManager::new();
        spawn_cmd(&mut m).await;
        assert!(m.child.is_some());
        m.shutdown_sync();
        assert!(m.child.is_none());
        assert_eq!(m.state, ProcState::Stopped);
    }
}
