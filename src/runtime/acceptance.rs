//! Host-side acceptance commands (provider-agnostic).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone)]
pub struct AcceptanceOutcome {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run `acceptance` shell command in `work_dir`.
///
/// Uses `sh -c` so plan authors can write normal shell one-liners.
pub async fn run_acceptance(
    work_dir: &Path,
    command: &str,
    timeout: Duration,
) -> Result<AcceptanceOutcome> {
    info!(cwd = %work_dir.display(), cmd = %command, "running acceptance");

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().context("spawn acceptance shell")?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("acceptance timed out after {:?}", timeout))?
        .context("wait acceptance")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    let ok = output.status.success();

    if !ok {
        bail!(
            "acceptance failed (exit {:?}): {}\n{}",
            code,
            stderr.chars().take(400).collect::<String>(),
            stdout.chars().take(200).collect::<String>()
        );
    }

    Ok(AcceptanceOutcome {
        ok,
        exit_code: code,
        stdout,
        stderr,
    })
}

/// Soft version that returns Outcome instead of bailing (for scheduler).
pub async fn run_acceptance_soft(
    work_dir: &Path,
    command: &str,
    timeout: Duration,
) -> AcceptanceOutcome {
    match run_acceptance(work_dir, command, timeout).await {
        Ok(o) => o,
        Err(e) => AcceptanceOutcome {
            ok: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("{e:#}"),
        },
    }
}
