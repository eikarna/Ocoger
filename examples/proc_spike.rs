//! Windows-specific process management spike.
//! Run: cargo run --example proc_spike

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1) Spawn child with piped stdout/stderr.
    let mut child = Command::new("cmd")
        .args(["/c", "echo hello-spike && echo log-line-2 && exit 42"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let out = child.stdout.take().expect("piped");
    let err = child.stderr.take().expect("piped");
    let mut out_lines = BufReader::new(out).lines();
    let mut err_lines = BufReader::new(err).lines();

    // 2) Read all available lines until EOF (process exits).
    let mut collected_out = Vec::new();
    let mut collected_err = Vec::new();
    loop {
        tokio::select! {
            l = out_lines.next_line() => match l {
                Ok(Some(line)) => collected_out.push(line),
                Ok(None) => break,          // EOF: writer closed
                Err(e) => { eprintln!("stdout read err: {e}"); break; }
            },
            l = err_lines.next_line() => match l {
                Ok(Some(line)) => collected_err.push(line),
                Ok(None) => break,
                Err(e) => { eprintln!("stderr read err: {e}"); break; }
            },
        }
        // After first iteration, one of them may have hit EOF. A single
        // 'break' inside select! loop is fine if we only care about
        // nonblocking streaming; for capture semantics (spike), use
        // child.wait() after first EOF so the other side may also drain.
        if collected_out.len() >= 2 {
            break;
        }
    }
    let status = child.wait().await?;
    println!("exit: {:?} code={:?}", status.success(), status.code());
    println!("stdout: {:?}", collected_out);
    println!("stderr: {:?}", collected_err);
    assert!(collected_out.iter().any(|l| l.contains("hello-spike")));

    // 3) kill() on Windows.
    let mut child2 = Command::new("cmd")
        .args(["/c", "timeout /t 5 /nobreak >nul"])
        .stdout(Stdio::null())
        .spawn()?;
    let pid = child2.id().expect("pid");
    println!("spawned pid={pid}");
    let kill_result = child2.kill().await;
    println!("kill(): {:?}", kill_result);
    let final_status = child2.wait().await?;
    println!("post-kill exit: {:?}", final_status);

    println!("SPIKE OK");
    Ok(())
}
