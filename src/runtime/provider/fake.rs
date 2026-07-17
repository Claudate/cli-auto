//! Fake provider for tests: inline stub (+ optional delayed bg).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::process::Command;

use super::{
    parse_claude_result_json, Capabilities, StartCtx, TaskResult, TaskStatus, WorkerHandle,
    WorkerProvider, WorkerStatus,
};
use crate::plan::TaskIR;

pub struct FakeProvider {
    bin: String,
}

impl FakeProvider {
    pub fn new(bin: String) -> Self {
        Self { bin }
    }

    fn write_success(task: &TaskIR, stdout_path: &PathBuf, meta_path: &PathBuf, mode: &str) -> Result<()> {
        // NDJSON stream-json shape so desktop pretty console can render without real Claude.
        let lines = vec![
            serde_json::json!({"type":"system","subtype":"init","provider":"fake"}),
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text": format!("Fake working on {}", task.id)}]}}),
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"path":"README.md"}}]}}),
            serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"1","content":"ok"}]}}),
            serde_json::json!({
                "type": "result",
                "subtype": "success",
                "result": format!("fake ok for {}", task.id),
                "session_id": format!("fake-session-{}", task.id),
                "total_cost_usd": 0.01,
            }),
        ];
        let mut body = String::new();
        for v in lines {
            body.push_str(&serde_json::to_string(&v)?);
            body.push('\n');
        }
        body.push_str("CCO_DONE ok\n");
        std::fs::write(stdout_path, body)?;
        std::fs::write(
            meta_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "exit_code": 0,
                "provider": "fake",
                "mode": mode,
                "inline": true,
                "agent_id": if mode == "bg" { Some(format!("fake-bg-{}", task.id)) } else { None },
            }))?,
        )?;
        Ok(())
    }
}

#[async_trait]
impl WorkerProvider for FakeProvider {
    fn name(&self) -> &str {
        "fake"
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
        if which::which(&self.bin).is_err()
            && self.bin != "inline"
            && self.bin != "fake-inline"
        {
            tracing::debug!(bin = %self.bin, "fake bin not in PATH; will use inline stub");
        }
        Ok(())
    }

    fn validate_task(&self, task: &TaskIR) -> Result<()> {
        if task.prompt.trim().is_empty() {
            bail!("empty prompt");
        }
        Ok(())
    }

    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle> {
        std::fs::create_dir_all(&ctx.task_dir)?;
        std::fs::write(ctx.task_dir.join("prompt.md"), &task.prompt)?;
        let stdout_path = ctx.task_dir.join("stdout.json");
        let meta_path = ctx.task_dir.join("meta.json");
        let done_flag = ctx.task_dir.join(".done");

        let mode = if task.mode == "bg" { "bg" } else { "print" };

        let handle = WorkerHandle {
            provider: "fake".into(),
            task_id: task.id.clone(),
            mode: mode.into(),
            opaque_id: if mode == "bg" {
                format!("agent:fake-bg-{}", task.id)
            } else {
                format!("fake:{}", task.id)
            },
            pid: None,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        // bg: delay completion so scheduler must poll
        if mode == "bg"
            && (self.bin == "inline"
                || self.bin == "fake-inline"
                || which::which(&self.bin).is_err())
        {
            Self::write_success(task, &stdout_path, &meta_path, "bg")?;
            let delay_ms: u64 = std::env::var("CCO_FAKE_BG_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(80);
            let done = done_flag.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                let _ = std::fs::write(&done, "0");
            });
            return Ok(handle);
        }

        if self.bin == "inline" || self.bin == "fake-inline" || which::which(&self.bin).is_err() {
            Self::write_success(task, &stdout_path, &meta_path, "print")?;
            std::fs::write(&done_flag, "0")?;
            return Ok(handle);
        }

        let mut cmd = Command::new(&self.bin);
        cmd.arg("-p")
            .arg("--bare")
            .arg("--output-format")
            .arg("json")
            .arg(&task.prompt);
        cmd.current_dir(&ctx.work_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = tokio::time::timeout(Duration::from_secs(30), cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("fake provider timeout"))?
            .map_err(|e| anyhow::anyhow!("spawn fake bin: {e}"))?;

        let code = output.status.code().unwrap_or(-1);
        std::fs::write(&stdout_path, &output.stdout)?;
        std::fs::write(ctx.task_dir.join("stderr.log"), &output.stderr)?;
        std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "exit_code": code,
                "provider": "fake",
            }))?,
        )?;
        std::fs::write(&done_flag, code.to_string())?;

        Ok(handle)
    }

    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus> {
        let done = handle
            .stdout_path
            .parent()
            .map(|p| p.join(".done"))
            .unwrap_or_else(|| PathBuf::from(".done"));
        if !done.exists() {
            return Ok(WorkerStatus::Running);
        }
        let code = std::fs::read_to_string(done)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(-1);
        Ok(match code {
            0 => WorkerStatus::Done,
            130 => WorkerStatus::Stopped,
            124 => WorkerStatus::Timeout,
            _ => WorkerStatus::Failed,
        })
    }

    async fn stop(&self, handle: &WorkerHandle) -> Result<()> {
        if let Some(parent) = handle.stdout_path.parent() {
            let _ = std::fs::write(parent.join(".done"), "130");
        }
        Ok(())
    }

    async fn collect(&self, handle: &WorkerHandle) -> Result<TaskResult> {
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap_or_default();
        let parsed = parse_claude_result_json(&stdout).unwrap_or(serde_json::json!({}));
        let code = handle
            .stdout_path
            .parent()
            .and_then(|p| std::fs::read_to_string(p.join(".done")).ok())
            .and_then(|s| s.trim().parse().ok());

        let status = match code {
            Some(0) => TaskStatus::Done,
            Some(130) => TaskStatus::Stopped,
            Some(124) => TaskStatus::Timeout,
            _ => TaskStatus::Failed,
        };

        let agent_id = handle
            .opaque_id
            .strip_prefix("agent:")
            .map(|s| s.to_string());

        Ok(TaskResult {
            status,
            exit_code: code,
            stdout_path: Some(handle.stdout_path.clone()),
            session_id: parsed
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            agent_id,
            cost_usd: parsed.get("total_cost_usd").and_then(|v| v.as_f64()),
            raw: parsed,
            error: None,
        })
    }
}
