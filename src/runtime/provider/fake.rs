//! Fake adapter implementing [`crate::ports::WorkerPort`] for tests and demos.
//!
//! [INPUT]: StartCtx · TaskIR（读 prompt 中 CCO_DONE）
//! [OUTPUT]: 立即/短延迟完成的 TaskResult
//! [POS]: 集成测试与 smoke 默认 provider
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::process::Command;

use super::{
    parse_claude_result_json, Capabilities, StartCtx, TaskResult, WorkerHandle, WorkerProvider,
    WorkerStatus,
};
use crate::plan::TaskIR;

pub struct FakeProvider {
    bin: String,
    /// Registry key / `WorkerProvider::name` (default `"fake"`).
    /// Tests may register aliases (`claude`, `codex`) that still run the inline stub.
    name: String,
}

impl FakeProvider {
    pub fn new(bin: String) -> Self {
        Self {
            bin,
            name: "fake".into(),
        }
    }

    /// Register this stub under an arbitrary provider name (e.g. `"claude"` / `"codex"`).
    pub fn with_name(bin: String, name: impl Into<String>) -> Self {
        Self {
            bin,
            name: name.into(),
        }
    }

    fn write_success(
        &self,
        task: &TaskIR,
        stdout_path: &PathBuf,
        meta_path: &PathBuf,
        mode: &str,
    ) -> Result<()> {
        // NDJSON stream-json shape so desktop pretty console can render without real Claude.
        let lines = vec![
            serde_json::json!({"type":"system","subtype":"init","provider": self.name}),
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
                "provider": self.name,
                "mode": mode,
                "inline": true,
                "agent_id": if mode == "bg" { Some(format!("{}-bg-{}", self.name, task.id)) } else { None },
            }))?,
        )?;
        Ok(())
    }
}

#[async_trait]
impl WorkerProvider for FakeProvider {
    fn name(&self) -> &str {
        &self.name
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
        // Clear stale completion so a reused task_dir cannot short-circuit poll().
        let _ = std::fs::remove_file(ctx.task_dir.join(".done"));
        std::fs::write(ctx.task_dir.join("prompt.md"), &task.prompt)?;
        let stdout_path = ctx.task_dir.join("stdout.json");
        let meta_path = ctx.task_dir.join("meta.json");
        let done_flag = ctx.task_dir.join(".done");

        let mode = if task.mode == "bg" { "bg" } else { "print" };

        let handle = WorkerHandle {
            provider: self.name.clone(),
            task_id: task.id.clone(),
            mode: mode.into(),
            opaque_id: if mode == "bg" {
                format!("agent:{}-bg-{}", self.name, task.id)
            } else {
                format!("{}:{}", self.name, task.id)
            },
            pid: None,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        // Test hooks (inline / missing bin only):
        //   CCO_FAKE_HANG                 — write partial log, never finish (stall patrol test)
        //   CCO_FAKE_HANG_UNTIL_FAILOVER  — hang while provider == "claude"; succeed after switch
        //   CCO_FAKE_FAIL_ONCE            — fail first start; succeed once attempt-1.* archive exists
        //   CCO_FAKE_STOP                 — finish immediately as user-stop (exit 130)
        let hang_until_failover = task.prompt.contains("CCO_FAKE_HANG_UNTIL_FAILOVER");
        let hang = task.prompt.contains("CCO_FAKE_HANG") && !hang_until_failover;
        let fail_once = task.prompt.contains("CCO_FAKE_FAIL_ONCE");
        let stop_now = task.prompt.contains("CCO_FAKE_STOP");
        let prior_fail = ctx.task_dir.join("attempt-1.stdout.json").exists()
            || ctx.task_dir.join("attempt-1.meta.json").exists();
        let inlineish = self.bin == "inline"
            || self.bin == "fake-inline"
            || which::which(&self.bin).is_err();

        if hang_until_failover && inlineish {
            // Hang only under the original house ("claude"); after H4 switches to "codex", succeed.
            if self.name == "claude" {
                let body = format!(
                    "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"fake hang-until-failover {id} on {prov}\"}}]}}}}\n",
                    id = task.id,
                    prov = self.name
                );
                std::fs::write(&stdout_path, body)?;
                std::fs::write(
                    &meta_path,
                    serde_json::to_string_pretty(&serde_json::json!({
                        "provider": self.name,
                        "mode": mode,
                        "hang_until_failover": true,
                    }))?,
                )?;
                // never write .done while still on claude
                return Ok(handle);
            }
            // Fallback house: normal success path below.
        }

        if hang && inlineish {
            // Partial log so patrol has a baseline, then freeze.
            let body = format!(
                "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"fake hang {id}\"}}]}}}}\n",
                id = task.id
            );
            std::fs::write(&stdout_path, body)?;
            std::fs::write(
                &meta_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "provider": self.name,
                    "mode": mode,
                    "hang": true,
                }))?,
            )?;
            // never write .done
            return Ok(handle);
        }

        if stop_now && inlineish {
            // Simulate user-initiated stop: exit 130 → WorkerStatus::Stopped.
            let body = format!(
                "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"fake stop {}\"}}]}}}}\n",
                task.id
            );
            std::fs::write(&stdout_path, body)?;
            std::fs::write(
                &meta_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "exit_code": 130,
                    "provider": self.name,
                    "mode": mode,
                    "stopped": true,
                }))?,
            )?;
            std::fs::write(&done_flag, "130")?;
            return Ok(handle);
        }

        if fail_once
            && !prior_fail
            && (self.bin == "inline"
                || self.bin == "fake-inline"
                || which::which(&self.bin).is_err())
        {
            std::fs::write(
                &stdout_path,
                format!(
                    "{{\"type\":\"result\",\"subtype\":\"error\",\"result\":\"fake fail once {}\"}}\n",
                    task.id
                ),
            )?;
            std::fs::write(
                &meta_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "exit_code": 1,
                    "provider": self.name,
                    "mode": mode,
                    "fail_once": true,
                }))?,
            )?;
            std::fs::write(&done_flag, "1")?;
            return Ok(handle);
        }

        // bg: delay completion so scheduler must poll
        if mode == "bg"
            && (self.bin == "inline"
                || self.bin == "fake-inline"
                || which::which(&self.bin).is_err())
        {
            self.write_success(task, &stdout_path, &meta_path, "bg")?;
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
            self.write_success(task, &stdout_path, &meta_path, "print")?;
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
                "provider": self.name,
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
        Ok(super::worker_status_from_exit(code))
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

        let status = super::task_status_from_exit(code);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::TaskIR;
    use crate::ports::worker::TaskStatus;
    use tempfile::tempdir;

    fn sample_task(id: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: format!("hello {id}"),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
        tags: vec![],
        }
    }

    #[tokio::test]
    async fn start_clears_stale_done_from_reused_task_dir() {
        let dir = tempdir().unwrap();
        let task_dir = dir.path().join("tasks").join("__chat__");
        std::fs::create_dir_all(&task_dir).unwrap();
        // Simulate a previous successful turn leaving completion + empty stdout
        // (the bug path that made chat soft-fallback to the local template).
        std::fs::write(task_dir.join(".done"), "0").unwrap();
        std::fs::write(task_dir.join("stdout.json"), "").unwrap();

        let provider = FakeProvider::new("inline".into());
        let task = sample_task("t1");
        let ctx = StartCtx {
            run_id: "chat-default".into(),
            project_root: dir.path().to_path_buf(),
            work_dir: dir.path().to_path_buf(),
            task_dir: task_dir.clone(),
            env_extra: vec![],
        };

        let handle = provider.start(&task, &ctx).await.unwrap();
        // Inline fake re-writes .done=0 after success; assert new content was written
        // (not the empty leftover) and collect sees Done with real stdout.
        assert!(task_dir.join(".done").is_file());
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
        assert!(
            stdout.contains("fake ok for t1"),
            "expected fresh stdout, got: {stdout:?}"
        );
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Done
        ));
        let result = provider.collect(&handle).await.unwrap();
        assert_eq!(result.status, TaskStatus::Done);
    }

    #[tokio::test]
    async fn start_clears_stale_done_so_poll_is_not_prematurely_done() {
        let dir = tempdir().unwrap();
        let task_dir = dir.path().join("tasks").join("__chat__");
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(task_dir.join(".done"), "0").unwrap();

        let provider = FakeProvider::new("inline".into());
        // Hang path never writes .done — after start, poll must be Running (stale cleared).
        let mut task = sample_task("hang");
        task.prompt = "CCO_FAKE_HANG freeze".into();
        let ctx = StartCtx {
            run_id: "chat-default".into(),
            project_root: dir.path().to_path_buf(),
            work_dir: dir.path().to_path_buf(),
            task_dir: task_dir.clone(),
            env_extra: vec![],
        };

        let handle = provider.start(&task, &ctx).await.unwrap();
        assert!(
            !task_dir.join(".done").exists(),
            "stale .done must be removed when the new run has not finished"
        );
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Running
        ));
    }
}
