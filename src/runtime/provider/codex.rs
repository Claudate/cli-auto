//! [INPUT]: 依赖本机 `codex` CLI、tokio process、WorkerProvider 契约
//! [OUTPUT]: 对外提供 CodexProvider（print/exec 非交互）
//! [POS]: runtime/provider 的 Codex 后端，与 ClaudeProvider 并列
//! [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

//! Codex CLI provider: non-interactive `codex exec` (print-equivalent).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::{Child, Command};
use tracing::{info, warn};

use super::{
    ensure_done_marker, parse_claude_result_json, Capabilities, StartCtx, TaskResult, TaskStatus,
    WorkerHandle, WorkerProvider, WorkerStatus,
};
use crate::plan::TaskIR;

pub struct CodexProvider {
    bin: String,
    extra_args: Vec<String>,
}

impl CodexProvider {
    pub fn new(bin: impl Into<String>, extra_args: Vec<String>) -> Self {
        Self {
            bin: bin.into(),
            extra_args,
        }
    }

    fn opt_str(opts: &serde_json::Value, key: &str) -> Option<String> {
        opts.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }

    fn opt_bool(opts: &serde_json::Value, key: &str) -> Option<bool> {
        opts.get(key).and_then(|v| v.as_bool())
    }

    fn apply_common_flags(&self, cmd: &mut Command, opts: &serde_json::Value) {
        // Prefer JSON-ish progressive output when available; ignore if unsupported at runtime.
        // Users can override entirely via extra_args / provider_opts.extra_args.
        let model = Self::opt_str(opts, "model");
        let full_auto = Self::opt_bool(opts, "full_auto").unwrap_or(true);
        let json = Self::opt_bool(opts, "json").unwrap_or(true);

        if full_auto {
            cmd.arg("--full-auto");
        }
        if json {
            // Newer codex builds accept --json on exec; if not, process still runs and we treat as text.
            cmd.arg("--json");
        }
        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }
        for a in &self.extra_args {
            cmd.arg(a);
        }
        if let Some(arr) = opts.get("extra_args").and_then(|v| v.as_array()) {
            for a in arr {
                if let Some(s) = a.as_str() {
                    cmd.arg(s);
                }
            }
        }
    }

    fn apply_env(cmd: &mut Command, ctx: &StartCtx) {
        for (k, v) in &ctx.env_extra {
            cmd.env(k, v);
        }
    }

    async fn start_exec(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        let prompt = task.prompt.clone();
        let _ = std::fs::write(&stdout_path, "");
        let _ = std::fs::write(
            &stderr_path,
            format!(
                "[{}] starting codex exec · task={} · cwd={}\n",
                chrono::Utc::now().to_rfc3339(),
                task.id,
                ctx.work_dir.display()
            ),
        );

        let mut cmd = Command::new(&self.bin);
        // Non-interactive path: `codex exec [flags] <prompt>`
        cmd.arg("exec");
        self.apply_common_flags(&mut cmd, &task.provider_opts);
        cmd.arg(&prompt);
        cmd.current_dir(&ctx.work_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        Self::apply_env(&mut cmd, ctx);

        info!(task = %task.id, bin = %self.bin, cwd = %ctx.work_dir.display(), "starting codex exec");

        let child: Child = cmd.spawn().with_context(|| format!("spawn {}", self.bin))?;
        let pid = child.id();
        let opaque = pid
            .map(|p| format!("pid:{p}"))
            .unwrap_or_else(|| format!("codex:{}", task.id));

        let timeout_secs = task
            .provider_opts
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60 * 30);
        let timeout = Duration::from_secs(timeout_secs);

        let handle = WorkerHandle {
            provider: "codex".into(),
            task_id: task.id.clone(),
            mode: "print".into(),
            opaque_id: opaque.clone(),
            pid,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        let stdout_path_c = stdout_path.clone();
        let stderr_path_c = stderr_path.clone();
        let meta_path_c = meta_path.clone();
        let done_flag = ctx.task_dir.join(".done");

        tokio::spawn(async move {
            let result = stream_child(child, timeout, &stdout_path_c, &stderr_path_c).await;
            match result {
                Ok(code) => {
                    let meta = serde_json::json!({
                        "provider": "codex",
                        "mode": "print",
                        "exit_code": code,
                        "opaque_id": opaque,
                    });
                    let _ = std::fs::write(
                        &meta_path_c,
                        serde_json::to_string_pretty(&meta).unwrap_or_default(),
                    );
                    let _ = std::fs::write(&done_flag, format!("{code}"));
                }
                Err(e) => {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&stderr_path_c)
                    {
                        let _ = writeln!(f, "{e:#}");
                    }
                    let meta = serde_json::json!({
                        "provider": "codex",
                        "error": format!("{e:#}"),
                        "exit_code": -1,
                        "mode": "print",
                    });
                    let _ = std::fs::write(
                        &meta_path_c,
                        serde_json::to_string_pretty(&meta).unwrap_or_default(),
                    );
                    let _ = std::fs::write(&done_flag, "-1");
                }
            }
        });

        let start_meta = serde_json::json!({
            "provider": "codex",
            "mode": "print",
            "pid": pid,
            "opaque_id": handle.opaque_id,
            "started_at": handle.started_at.to_rfc3339(),
            "prompt_bytes": prompt.len(),
        });
        std::fs::write(&meta_path, serde_json::to_string_pretty(&start_meta)?)?;
        Ok(handle)
    }
}

#[async_trait]
impl WorkerProvider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            print: true,
            background: false,
            stop: true,
            cost: false,
            session_resume: false,
            interactive_pty: false,
        }
    }

    async fn preflight(&self) -> Result<()> {
        let path = which::which(&self.bin).with_context(|| {
            format!(
                "codex binary not found: {} (install OpenAI Codex CLI or set CCO_CODEX_BIN)",
                self.bin
            )
        })?;
        // Best-effort version probe; do not fail hard on unknown flags.
        let out = Command::new(&path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                // Some builds use `codex version`
                let out2 = Command::new(&path)
                    .arg("version")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await;
                if out2.map(|x| x.status.success()).unwrap_or(false) {
                    Ok(())
                } else if path.exists() {
                    warn!(bin = %self.bin, stderr = %err, "codex version probe weak; binary exists");
                    Ok(())
                } else {
                    bail!("codex preflight failed: {err}");
                }
            }
            Err(e) => bail!("codex preflight spawn failed: {e}"),
        }
    }

    fn validate_task(&self, task: &TaskIR) -> Result<()> {
        if task.prompt.trim().is_empty() {
            bail!("codex task requires non-empty prompt");
        }
        if task.mode == "bg" {
            bail!("codex provider does not support mode=bg yet (use print/exec)");
        }
        Ok(())
    }

    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle> {
        let stdout_path = ctx.task_dir.join("stdout.json");
        let stderr_path = ctx.task_dir.join("stderr.log");
        let meta_path = ctx.task_dir.join("meta.json");
        self.start_exec(task, ctx, stdout_path, stderr_path, meta_path)
            .await
    }

    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus> {
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
            return Ok(match code {
                0 => WorkerStatus::Done,
                124 => WorkerStatus::Timeout,
                130 => WorkerStatus::Stopped,
                _ => WorkerStatus::Failed,
            });
        }
        if let Some(pid) = handle.pid {
            if process_alive(pid) {
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

    async fn stop(&self, handle: &WorkerHandle) -> Result<()> {
        if let Some(pid) = handle.pid {
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
            let done = handle
                .stdout_path
                .parent()
                .map(|p| p.join(".done").exists())
                .unwrap_or(false);
            if done {
                break;
            }
            if let Some(pid) = handle.pid {
                if !process_alive(pid) {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap_or_default();
        let meta_text = std::fs::read_to_string(&handle.meta_path).unwrap_or_default();
        let meta: serde_json::Value =
            serde_json::from_str(&meta_text).unwrap_or(serde_json::json!({}));

        let exit_code = meta
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32)
            .or_else(|| {
                handle
                    .stdout_path
                    .parent()
                    .and_then(|p| std::fs::read_to_string(p.join(".done")).ok())
                    .and_then(|s| s.trim().parse().ok())
            });

        // Reuse lenient JSON finder; works for codex --json last object too.
        let parsed = parse_claude_result_json(&stdout).ok();
        let session_id = parsed
            .as_ref()
            .and_then(|v| {
                v.get("session_id")
                    .or_else(|| v.get("thread_id"))
                    .or_else(|| v.get("id"))
                    .and_then(|x| x.as_str())
            })
            .map(|s| s.to_string());

        let status = match exit_code {
            Some(0) => TaskStatus::Done,
            Some(124) => TaskStatus::Timeout,
            Some(130) => TaskStatus::Stopped,
            Some(_) => TaskStatus::Failed,
            None if ensure_done_marker(&stdout) => TaskStatus::Done,
            None => TaskStatus::Failed,
        };

        let error = if status == TaskStatus::Failed {
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

        Ok(TaskResult {
            status,
            exit_code,
            stdout_path: Some(handle.stdout_path.clone()),
            session_id,
            agent_id: None,
            cost_usd: None,
            raw: parsed.unwrap_or(meta),
            error,
        })
    }
}

async fn stream_child(
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
    // Append to stderr so start banner remains.
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

fn process_alive(pid: u32) -> bool {
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

#[cfg(unix)]
unsafe fn libc_kill(pid: i32, sig: i32) -> i32 {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, sig)
}
