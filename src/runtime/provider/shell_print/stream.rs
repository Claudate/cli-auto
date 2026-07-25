//! Stream child stdout/stderr to task logs; process liveness helpers.
//!
//! [INPUT]: Child · timeout · log paths
//! [OUTPUT]: exit code (124 on timeout)
//! [POS]: runtime/provider/shell_print
//! [PROTOCOL]: stderr append mode keeps start banner (codex/gemini pattern)

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::Child;

/// Pump process stdout/stderr into log files while the child runs.
/// Returns the process exit code (124 on timeout).
///
/// Stdout is truncated on open; stderr is **appended** so the start banner remains.
pub async fn stream_child(
    mut child: Child,
    timeout: Duration,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<i32> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut out_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(stdout_path)
        .await
        .with_context(|| format!("open stdout {}", stdout_path.display()))?;
    let mut err_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(stderr_path)
        .await
        .with_context(|| format!("open stderr {}", stderr_path.display()))?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out_buf = [0u8; 8192];
    let mut err_buf = [0u8; 8192];
    let mut out_open = stdout.is_some();
    let mut err_open = stderr.is_some();

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if !out_open && !err_open {
            break;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Ok(124);
        }

        tokio::select! {
            biased;
            n = async {
                if let Some(r) = stdout.as_mut() {
                    r.read(&mut out_buf).await
                } else {
                    std::future::pending().await
                }
            }, if out_open => {
                match n {
                    Ok(0) | Err(_) => out_open = false,
                    Ok(n) => {
                        let _ = out_file.write_all(&out_buf[..n]).await;
                        let _ = out_file.flush().await;
                    }
                }
            }
            n = async {
                if let Some(r) = stderr.as_mut() {
                    r.read(&mut err_buf).await
                } else {
                    std::future::pending().await
                }
            }, if err_open => {
                match n {
                    Ok(0) | Err(_) => err_open = false,
                    Ok(n) => {
                        let _ = err_file.write_all(&err_buf[..n]).await;
                        let _ = err_file.flush().await;
                    }
                }
            }
            _ = tokio::time::sleep(remaining) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Ok(124);
            }
        }
    }

    let wait = tokio::time::timeout(
        deadline.saturating_duration_since(tokio::time::Instant::now()),
        child.wait(),
    )
    .await;
    match wait {
        Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
        Ok(Err(e)) => {
            let _ = child.start_kill();
            bail!("wait error: {e}");
        }
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Ok(124)
        }
    }
}

pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc_kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// SIGTERM then SIGKILL (unix) or taskkill (windows).
pub async fn stop_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            let _ = libc_kill(pid as i32, 15);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        if process_alive(pid) {
            unsafe {
                let _ = libc_kill(pid as i32, 9);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .await;
    }
}

#[cfg(unix)]
unsafe fn libc_kill(pid: i32, sig: i32) -> i32 {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, sig)
}
