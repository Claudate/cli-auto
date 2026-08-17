//! SDK / non-CLI worker adapter implementing [`crate::ports::WorkerPort`] (P2-7 S0+S1).
//!
//! [INPUT]: StartCtx · TaskIR · optional SdkBackend
//! [OUTPUT]: in-process TaskResult (no agent CLI spawn)
//! [POS]: runtime/provider — proves non-CLI path; default registry **off**
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: S0 = InlineSdkBackend；S1 HTTP = [`super::sdk_http`]；S2 tools = [`super::sdk_tool_loop`]（均注入，不堆进本文件）

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use async_trait::async_trait;

use super::{
    parse_claude_result_json, Capabilities, StartCtx, TaskResult, WorkerHandle, WorkerPort,
    WorkerStatus,
};
use crate::plan::TaskIR;

/// Transport behind [`SdkProvider`]. S0 = inline; S1 = Messages HTTP; S2 = tool loop.
#[async_trait]
pub trait SdkBackend: Send + Sync {
    /// Human label for meta.json (not the registry provider name).
    fn kind(&self) -> &str;

    /// Optional readiness check (S1: API key present). Default: ready.
    async fn preflight(&self) -> Result<()> {
        Ok(())
    }

    /// Run one task in-process. Write `stdout_path` (NDJSON) and return exit code.
    async fn execute(&self, task: &TaskIR, ctx: &StartCtx, stdout_path: &PathBuf) -> Result<i32>;
}

/// In-process stub: no network, no CLI. Proves WorkerPort without process spawn.
pub struct InlineSdkBackend;

#[async_trait]
impl SdkBackend for InlineSdkBackend {
    fn kind(&self) -> &str {
        "inline"
    }

    async fn execute(&self, task: &TaskIR, _ctx: &StartCtx, stdout_path: &PathBuf) -> Result<i32> {
        // Tiny yield so start/poll interleaving is realistic under load.
        tokio::task::yield_now().await;

        if task.prompt.contains("CCO_SDK_FAIL") {
            let body = format!(
                "{{\"type\":\"result\",\"subtype\":\"error\",\"result\":\"sdk inline fail {}\"}}\n",
                task.id
            );
            std::fs::write(stdout_path, body)?;
            return Ok(1);
        }

        let lines = vec![
            serde_json::json!({"type":"system","subtype":"init","provider":"sdk","backend":"inline"}),
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "content": [{
                        "type": "text",
                        "text": format!("SDK inline working on {}", task.id)
                    }]
                }
            }),
            serde_json::json!({
                "type": "result",
                "subtype": "success",
                "result": format!("sdk ok for {}", task.id),
                "session_id": format!("sdk-session-{}", task.id),
                "total_cost_usd": 0.0,
            }),
        ];
        let mut body = String::new();
        for v in lines {
            body.push_str(&serde_json::to_string(&v)?);
            body.push('\n');
        }
        body.push_str("CCO_DONE ok\n");
        std::fs::write(stdout_path, body)?;
        Ok(0)
    }
}

/// Non-CLI provider registered as `"sdk"` when config enables it.
pub struct SdkProvider {
    backend: Arc<dyn SdkBackend>,
}

impl SdkProvider {
    pub fn new() -> Self {
        Self {
            backend: Arc::new(InlineSdkBackend),
        }
    }

    pub fn with_backend(backend: Arc<dyn SdkBackend>) -> Self {
        Self { backend }
    }
}

impl Default for SdkProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkerPort for SdkProvider {
    fn name(&self) -> &str {
        "sdk"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            print: true,
            background: false,
            stop: true,
            cost: true,
            session_resume: false,
            interactive_pty: false,
        }
    }

    async fn preflight(&self) -> Result<()> {
        self.backend.preflight().await
    }

    fn validate_task(&self, task: &TaskIR) -> Result<()> {
        if task.prompt.trim().is_empty() {
            bail!("empty prompt");
        }
        // S0: print/auto only (no bg agent process).
        if !matches!(task.mode.as_str(), "print" | "auto" | "") {
            bail!(
                "unsupported mode for sdk (S0 print-only): {} — use print/auto",
                task.mode
            );
        }
        Ok(())
    }

    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle> {
        std::fs::create_dir_all(&ctx.task_dir)?;
        let _ = std::fs::remove_file(ctx.task_dir.join(".done"));
        std::fs::write(ctx.task_dir.join("prompt.md"), &task.prompt)?;

        let stdout_path = ctx.task_dir.join("stdout.json");
        let meta_path = ctx.task_dir.join("meta.json");
        let done_flag = ctx.task_dir.join(".done");
        let mode = "print";

        let handle = WorkerHandle {
            provider: "sdk".into(),
            task_id: task.id.clone(),
            mode: mode.into(),
            opaque_id: format!("sdk:{}", task.id),
            pid: None,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        // Optional delayed finish for poll realism (tests / demos).
        let delay_ms: u64 = std::env::var("CCO_SDK_INLINE_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if delay_ms > 0 {
            let backend = Arc::clone(&self.backend);
            let task = task.clone();
            let ctx = ctx.clone();
            let stdout_path = stdout_path.clone();
            let meta_path = meta_path.clone();
            let done_flag = done_flag.clone();
            let backend_kind = backend.kind().to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                let code = match backend.execute(&task, &ctx, &stdout_path).await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = std::fs::write(
                            &stdout_path,
                            format!(
                                "{{\"type\":\"result\",\"subtype\":\"error\",\"result\":\"{e}\"}}\n"
                            ),
                        );
                        1
                    }
                };
                let inline = backend_kind == "inline";
                let _ = std::fs::write(
                    &meta_path,
                    serde_json::to_string_pretty(&serde_json::json!({
                        "exit_code": code,
                        "provider": "sdk",
                        "mode": "print",
                        "backend": backend_kind,
                        "inline_sdk": inline,
                    }))
                    .unwrap_or_else(|_| "{}".into()),
                );
                let _ = std::fs::write(&done_flag, code.to_string());
            });
            return Ok(handle);
        }

        let code = self.backend.execute(task, ctx, &stdout_path).await?;
        let backend_kind = self.backend.kind();
        std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "exit_code": code,
                "provider": "sdk",
                "mode": mode,
                "backend": backend_kind,
                "inline_sdk": backend_kind == "inline",
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
        // Cooperative stop for delayed backend; no OS process to kill in S0.
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

        Ok(TaskResult {
            status,
            exit_code: code,
            stdout_path: Some(handle.stdout_path.clone()),
            session_id: parsed
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            agent_id: None,
            cost_usd: parsed.get("total_cost_usd").and_then(|v| v.as_f64()),
            raw: parsed,
            error: None,
            done_marker: code == Some(0),
            execution_evidence: status.is_success(),
            platform_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::TaskIR;
    use crate::ports::worker::TaskStatus;
    use tempfile::tempdir;

    fn sample_task(id: &str, prompt: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "sdk".into(),
            mode: "print".into(),
            prompt: prompt.into(),
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
            wait_for: vec![],
        }
    }

    fn ctx(dir: &std::path::Path) -> StartCtx {
        let task_dir = dir.join("tasks").join("t1");
        std::fs::create_dir_all(&task_dir).unwrap();
        StartCtx {
            run_id: "run-sdk".into(),
            project_root: dir.to_path_buf(),
            work_dir: dir.to_path_buf(),
            task_dir,
            env_extra: vec![],
        }
    }

    #[tokio::test]
    async fn start_poll_collect_success_no_cli_spawn() {
        let dir = tempdir().unwrap();
        let provider = SdkProvider::new();
        let task = sample_task("t1", "hello sdk");
        let start_ctx = ctx(dir.path());

        assert_eq!(provider.name(), "sdk");
        provider.preflight().await.unwrap();
        provider.validate_task(&task).unwrap();

        let handle = provider.start(&task, &start_ctx).await.unwrap();
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Done
        ));
        let result = provider.collect(&handle).await.unwrap();
        assert_eq!(result.status, TaskStatus::Done);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.session_id.as_deref(), Some("sdk-session-t1"));
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
        assert!(stdout.contains("CCO_DONE"), "stdout: {stdout}");
        assert!(stdout.contains("sdk ok for t1"));
        assert!(start_ctx.task_dir.join("prompt.md").is_file());
        assert!(handle.pid.is_none(), "sdk has no process pid");
    }

    #[tokio::test]
    async fn start_clears_stale_done() {
        let dir = tempdir().unwrap();
        let start_ctx = ctx(dir.path());
        std::fs::write(start_ctx.task_dir.join(".done"), "0").unwrap();
        std::fs::write(start_ctx.task_dir.join("stdout.json"), "").unwrap();

        let provider = SdkProvider::new();
        let task = sample_task("t-stale", "rewrite me");
        // Immediate success must replace empty leftover stdout (same contract as fake/claude).
        let handle = provider.start(&task, &start_ctx).await.unwrap();
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
        assert!(
            stdout.contains("sdk ok"),
            "stale empty stdout must be replaced: {stdout:?}"
        );
        assert!(start_ctx.task_dir.join(".done").is_file());
    }

    #[tokio::test]
    async fn fail_prompt_hook() {
        let dir = tempdir().unwrap();
        let provider = SdkProvider::new();
        let task = sample_task("f1", "CCO_SDK_FAIL boom");
        let start_ctx = ctx(dir.path());
        let handle = provider.start(&task, &start_ctx).await.unwrap();
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Failed
        ));
        let result = provider.collect(&handle).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn reject_bg_mode_in_s0() {
        let provider = SdkProvider::new();
        let mut task = sample_task("bg", "x");
        task.mode = "bg".into();
        assert!(provider.validate_task(&task).is_err());
    }

    #[tokio::test]
    async fn reject_empty_prompt() {
        let provider = SdkProvider::new();
        let task = sample_task("e", "   ");
        assert!(provider.validate_task(&task).is_err());
    }
}
