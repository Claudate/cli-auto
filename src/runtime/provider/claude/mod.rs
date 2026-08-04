//! Claude CLI adapter implementing [`crate::ports::WorkerPort`].
//!
//! [INPUT]: StartCtx · TaskIR · config bin/auth
//! [OUTPUT]: spawn/poll/collect · stream-json 解析 · agent_id
//! [POS]: 默认真实 provider；D4 已目录化 spawn/poll_bg/parse_result
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md

mod parse_result;
mod poll_bg;
mod spawn;

pub use parse_result::parse_agent_id;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::warn;

use super::{
    ensure_done_marker, parse_claude_result_json, Capabilities, StartCtx, TaskResult, TaskStatus,
    WorkerHandle, WorkerProvider, WorkerStatus,
};
use crate::plan::TaskIR;

pub struct ClaudeProvider {
    bin: String,
    extra_args: Vec<String>,
}

// method impls in spawn.rs / poll_bg.rs

#[async_trait]
impl WorkerProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            print: true,
            background: true,
            stop: true,
            cost: true,
            session_resume: false,
            interactive_pty: false,
        }
    }

    async fn preflight(&self) -> Result<()> {
        let path = if std::path::Path::new(&self.bin).is_file() {
            std::path::PathBuf::from(&self.bin)
        } else if let Some(found) = crate::runtime::provider::resolve_bin_on_disk(&self.bin) {
            std::path::PathBuf::from(found)
        } else {
            which::which(&self.bin).with_context(|| {
                format!(
                    "claude bin not found ({}). Install Claude CLI or set CCO_CLAUDE_BIN / providers.claude.bin",
                    self.bin
                )
            })?
        };
        let out = Command::new(&path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("run {} --version", path.display()))?;
        if !out.status.success() {
            bail!(
                "{} --version failed: {}",
                self.bin,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn validate_task(&self, task: &TaskIR) -> Result<()> {
        if task.prompt.trim().is_empty() {
            bail!("empty prompt");
        }
        if !matches!(task.mode.as_str(), "print" | "auto" | "bg") {
            bail!("unsupported mode for claude: {}", task.mode);
        }
        Ok(())
    }

    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle> {
        std::fs::create_dir_all(&ctx.task_dir)?;
        // Chat/planner reuse fixed dirs (`__chat__` / `__planner__`). A leftover
        // `.done` from a previous turn makes poll() return Done immediately with
        // empty/truncated stdout → soft-fallback to the local plan template.
        let _ = std::fs::remove_file(ctx.task_dir.join(".done"));
        std::fs::write(ctx.task_dir.join("prompt.md"), &task.prompt)?;

        let stdout_path = ctx.task_dir.join("stdout.json");
        let stderr_path = ctx.task_dir.join("stderr.log");
        let meta_path = ctx.task_dir.join("meta.json");

        let mode = if task.mode == "auto" {
            "print"
        } else {
            task.mode.as_str()
        };

        if mode == "bg" {
            self.start_bg(task, ctx, stdout_path, stderr_path, meta_path)
                .await
        } else {
            self.start_print(task, ctx, stdout_path, stderr_path, meta_path)
                .await
        }
    }

    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus> {
        if handle.mode == "bg" {
            let done_flag = handle
                .stdout_path
                .parent()
                .map(|p| p.join(".done"))
                .unwrap_or_else(|| PathBuf::from(".done"));
            if done_flag.exists() {
                let code = std::fs::read_to_string(&done_flag)
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(-1);
                return Ok(super::worker_status_from_exit(code));
            }

            let meta: serde_json::Value = std::fs::read_to_string(&handle.meta_path)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();

            if Self::bg_deadline_passed(&meta) {
                if let Some(id) = Self::agent_id_from_handle(handle) {
                    let _ = Command::new(&self.bin).args(["stop", &id]).output().await;
                }
                let _ = std::fs::write(&done_flag, "124");
                return Ok(WorkerStatus::Timeout);
            }

            let Some(agent_id) = Self::agent_id_from_handle(handle) else {
                return Ok(WorkerStatus::Failed);
            };

            self.refresh_bg_logs(handle, &agent_id).await;

            // completion markers in logs
            let logs = std::fs::read_to_string(&handle.stdout_path).unwrap_or_default();
            if ensure_done_marker(&logs) {
                let _ = std::fs::write(&done_flag, "0");
                return Ok(WorkerStatus::Done);
            }

            match self.agents_json().await {
                Ok(v) => {
                    if let Some(st) = poll_bg::find_agent_state(&v, &agent_id) {
                        let norm = st.to_ascii_lowercase();
                        if matches!(
                            norm.as_str(),
                            "done" | "completed" | "complete" | "success" | "finished"
                        ) {
                            let _ = std::fs::write(&done_flag, "0");
                            return Ok(WorkerStatus::Done);
                        }
                        if matches!(norm.as_str(), "failed" | "error" | "crashed" | "dead") {
                            let _ = std::fs::write(&done_flag, "1");
                            return Ok(WorkerStatus::Failed);
                        }
                        if matches!(norm.as_str(), "stopped" | "cancelled" | "canceled") {
                            let _ = std::fs::write(&done_flag, "130");
                            return Ok(WorkerStatus::Stopped);
                        }
                        // idle / running / active / pending
                        return Ok(WorkerStatus::Running);
                    }
                    // agent disappeared from list — treat as done if logs have content, else failed
                    if !logs.trim().is_empty() {
                        warn!(agent_id, "agent not in list; treating as done");
                        let _ = std::fs::write(&done_flag, "0");
                        Ok(WorkerStatus::Done)
                    } else {
                        Ok(WorkerStatus::Running)
                    }
                }
                Err(e) => {
                    warn!(error = %e, "agents --json failed; keep running");
                    Ok(WorkerStatus::Running)
                }
            }
        } else {
            // print mode
            let done_flag = handle
                .stdout_path
                .parent()
                .map(|p| p.join(".done"))
                .unwrap_or_else(|| PathBuf::from(".done"));
            if done_flag.exists() {
                let code = std::fs::read_to_string(&done_flag)
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(-1);
                Ok(super::worker_status_from_exit(code))
            } else if let Some(pid) = handle.pid {
                if poll_bg::process_alive(pid) {
                    Ok(WorkerStatus::Running)
                } else {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if done_flag.exists() {
                        return self.poll(handle).await;
                    }
                    Ok(WorkerStatus::Failed)
                }
            } else {
                Ok(WorkerStatus::Running)
            }
        }
    }

    async fn stop(&self, handle: &WorkerHandle) -> Result<()> {
        if handle.mode == "bg" {
            if let Some(id) = Self::agent_id_from_handle(handle) {
                let _ = Command::new(&self.bin).args(["stop", &id]).output().await;
            }
            if let Some(parent) = handle.stdout_path.parent() {
                let _ = std::fs::write(parent.join(".done"), "130");
            }
            return Ok(());
        }
        if let Some(pid) = handle.pid {
            #[cfg(unix)]
            {
                unsafe {
                    let _ = poll_bg::libc_kill(pid as i32, 15);
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
                if poll_bg::process_alive(pid) {
                    unsafe {
                        let _ = poll_bg::libc_kill(pid as i32, 9);
                    }
                }
            }
            #[cfg(not(unix))]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .status()
                    .await;
            }
            if let Some(parent) = handle.stdout_path.parent() {
                let _ = std::fs::write(parent.join(".done"), "130");
            }
        }
        Ok(())
    }

    async fn collect(&self, handle: &WorkerHandle) -> Result<TaskResult> {
        for _ in 0..50 {
            if matches!(
                self.poll(handle).await?,
                WorkerStatus::Done
                    | WorkerStatus::Failed
                    | WorkerStatus::Timeout
                    | WorkerStatus::Stopped
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if handle.mode == "bg" {
            if let Some(id) = Self::agent_id_from_handle(handle) {
                self.refresh_bg_logs(handle, &id).await;
            }
        }

        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap_or_default();
        let meta_text = std::fs::read_to_string(&handle.meta_path).unwrap_or_default();
        let meta: serde_json::Value =
            serde_json::from_str(&meta_text).unwrap_or(serde_json::json!({}));

        let meta_code = meta
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);
        let done_code = handle
            .stdout_path
            .parent()
            .and_then(|p| std::fs::read_to_string(p.join(".done")).ok())
            .and_then(|s| s.trim().parse().ok());
        // Prefer .done=130 (stop_run) over stream meta=-1 (SIGKILL race).
        let exit_code = super::resolve_exit_code(meta_code, done_code);

        let parsed = parse_claude_result_json(&stdout).ok();
        let session_id = parsed
            .as_ref()
            .and_then(|v| v.get("session_id").and_then(|x| x.as_str()))
            .map(|s| s.to_string());
        let agent_id = Self::agent_id_from_handle(handle);
        let cost = parsed.as_ref().and_then(|v| {
            v.get("total_cost_usd")
                .or_else(|| v.get("cost_usd"))
                .and_then(|x| x.as_f64())
        });

        let mut status = match exit_code {
            None if handle.mode == "bg" && ensure_done_marker(&stdout) => TaskStatus::Done,
            other => super::task_status_from_exit(other),
        };

        // dontAsk / missing allow: CLI returns exit 0 but tools were denied → false Done.
        // Count permission_denials on the final result object and demote to Failed.
        let denial_n = parsed
            .as_ref()
            .and_then(|v| v.get("permission_denials"))
            .and_then(|d| d.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let mut error = if status == TaskStatus::Failed {
            let stderr = handle
                .stdout_path
                .parent()
                .map(|p| std::fs::read_to_string(p.join("stderr.log")).unwrap_or_default())
                .unwrap_or_default();
            Some(if stderr.is_empty() {
                format!("exit {:?}", exit_code)
            } else {
                stderr.chars().take(500).collect()
            })
        } else {
            None
        };
        if denial_n > 0 && status == TaskStatus::Done {
            status = TaskStatus::Failed;
            error = Some(format!(
                "permission denied: {denial_n} tool call(s) blocked (permission_mode cannot auto-write; set bypassPermissions or authorize before run)"
            ));
        }

        Ok(TaskResult {
            status,
            exit_code,
            stdout_path: Some(handle.stdout_path.clone()),
            session_id,
            agent_id,
            cost_usd: cost,
            raw: parsed.unwrap_or(meta),
            error,
        })
    }
}
