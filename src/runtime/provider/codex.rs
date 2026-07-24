//! Codex CLI adapter implementing [`crate::ports::WorkerPort`] (second real provider).
//!
//! [INPUT]: StartCtx · TaskIR · config bin
//! [OUTPUT]: spawn/poll/collect
//! [POS]: 已实现；勿再写「尚无第二 provider」；P1-6 start 注入 cwd/scope 前缀
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: Codex 无 tool allowlist / --append-system-prompt → 前缀拼进 prompt

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
use crate::plan::{TaskIR, TaskScope};

/// Build cwd/scope lock text prepended to the Codex prompt (P1-6).
///
/// Codex has no tool allowlist and no `--append-system-prompt`, so the host
/// injects the same class of constraints Claude gets via system prompt as a
/// **prompt prefix**. Pure function — unit-testable without spawning codex.
pub fn build_scope_prefix(work_dir: &Path, scope: Option<&TaskScope>) -> String {
    let dir = work_dir.display();
    let mut parts = vec![format!(
        "CCO scope lock: work ONLY inside `{dir}`. Never read, list, search, or write outside this project directory. FORBIDDEN: home (~), Desktop, Documents, Downloads, Pictures, Movies, Music, Photos, and any absolute path not under `{dir}`. Do NOT run `find ~`, `ls ~`, `find /Users`, or any home-wide scan. Prefer relative paths from cwd."
    )];

    if let Some(s) = scope {
        if !s.paths.is_empty() {
            parts.push(format!(
                "Writable whitelist (scope.paths): {}. Do not write outside these globs (relative to project root).",
                s.paths.join(", ")
            ));
        }
        if !s.readonly.is_empty() {
            parts.push(format!(
                "Extra readonly ranges (scope.readonly): {}.",
                s.readonly.join(", ")
            ));
        }
        if !s.forbid.is_empty() {
            parts.push(format!(
                "Hard forbid (scope.forbid): {}. Never read, list, search, or write these paths.",
                s.forbid.join(", ")
            ));
        }
    }

    parts.join("\n")
}

/// Prepend scope lock to the user prompt for Codex exec.
pub fn with_scope_prefix(prompt: &str, work_dir: &Path, scope: Option<&TaskScope>) -> String {
    let prefix = build_scope_prefix(work_dir, scope);
    if prompt.trim().is_empty() {
        prefix
    } else {
        format!("{prefix}\n\n{prompt}")
    }
}

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
        // GUI/.app PATH often lacks node; codex shebang needs Homebrew bins.
        super::apply_worker_process_env(cmd, &ctx.env_extra);
    }

    async fn start_exec(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        // P1-6: Codex has no tool allowlist / append-system-prompt — inject
        // cwd/scope constraints as a prompt prefix (same class as Claude).
        let prompt = with_scope_prefix(&task.prompt, &ctx.work_dir, task.scope.as_ref());
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
                    let code = super::finalize_stream_exit(&done_flag, code);
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
                    let code = super::finalize_stream_exit(&done_flag, -1);
                    let meta = serde_json::json!({
                        "provider": "codex",
                        "error": format!("{e:#}"),
                        "exit_code": code,
                        "mode": "print",
                    });
                    let _ = std::fs::write(
                        &meta_path_c,
                        serde_json::to_string_pretty(&meta).unwrap_or_default(),
                    );
                    let _ = std::fs::write(&done_flag, format!("{code}"));
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
        // Inject GUI-safe PATH so `#!/usr/bin/env node` shebang resolves.
        let path_env = super::worker_path_env();
        let out = Command::new(&path)
            .arg("--version")
            .env("PATH", &path_env)
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
                    .env("PATH", &path_env)
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
        std::fs::create_dir_all(&ctx.task_dir)?;
        // Same as Claude: reused task_dir must not keep a prior completion marker.
        let _ = std::fs::remove_file(ctx.task_dir.join(".done"));
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
            return Ok(super::worker_status_from_exit(code));
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

        let meta_code = meta
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);
        let done_code = handle
            .stdout_path
            .parent()
            .and_then(|p| std::fs::read_to_string(p.join(".done")).ok())
            .and_then(|s| s.trim().parse().ok());
        let exit_code = super::resolve_exit_code(meta_code, done_code);

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
            None if ensure_done_marker(&stdout) => TaskStatus::Done,
            other => super::task_status_from_exit(other),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scope_prefix_locks_cwd_and_forbids_home() {
        let work = PathBuf::from("/tmp/proj");
        let prefix = build_scope_prefix(&work, None);
        assert!(
            prefix.contains("CCO scope lock: work ONLY inside `/tmp/proj`"),
            "missing cwd lock: {prefix}"
        );
        assert!(prefix.contains("FORBIDDEN: home (~)"));
        assert!(prefix.contains("Desktop"));
        assert!(prefix.contains("Do NOT run `find ~`"));
        assert!(
            !prefix.contains("scope.paths") && !prefix.contains("Writable whitelist"),
            "no paths → no whitelist line"
        );
        assert!(!prefix.contains("scope.forbid") && !prefix.contains("Hard forbid"));
    }

    #[test]
    fn scope_prefix_includes_paths_whitelist_and_forbid() {
        let work = PathBuf::from("/Users/me/project");
        let scope = TaskScope {
            paths: vec!["src/module_a/**".into(), ".cco-out/feat-a/**".into()],
            readonly: vec!["docs/**".into()],
            forbid: vec!["src/module_b/**".into(), "~".into()],
        };
        let prefix = build_scope_prefix(&work, Some(&scope));
        assert!(prefix.contains("CCO scope lock: work ONLY inside `/Users/me/project`"));
        assert!(
            prefix.contains(
                "Writable whitelist (scope.paths): src/module_a/**, .cco-out/feat-a/**"
            ),
            "missing paths whitelist: {prefix}"
        );
        assert!(
            prefix.contains("Extra readonly ranges (scope.readonly): docs/**"),
            "missing readonly: {prefix}"
        );
        assert!(
            prefix.contains("Hard forbid (scope.forbid): src/module_b/**, ~"),
            "missing forbid: {prefix}"
        );
        assert!(prefix.contains("Never read, list, search, or write these paths"));
    }

    #[test]
    fn with_scope_prefix_prepends_lock_before_user_prompt() {
        let work = PathBuf::from("/tmp/app");
        let scope = TaskScope {
            paths: vec!["src/**".into()],
            readonly: vec![],
            forbid: vec!["secrets/**".into()],
        };
        let out = with_scope_prefix("implement feature X\nCCO_DONE ok", &work, Some(&scope));
        assert!(
            out.starts_with("CCO scope lock:"),
            "prefix must lead: {}",
            &out[..out.len().min(80)]
        );
        assert!(out.contains("Writable whitelist (scope.paths): src/**"));
        assert!(out.contains("Hard forbid (scope.forbid): secrets/**"));
        assert!(
            out.contains("\n\nimplement feature X\nCCO_DONE ok"),
            "user prompt must follow blank line"
        );
        let lock_at = out.find("CCO scope lock:").unwrap();
        let body_at = out.find("implement feature X").unwrap();
        assert!(lock_at < body_at);
    }

    #[test]
    fn with_scope_prefix_empty_prompt_is_just_lock() {
        let work = PathBuf::from("/tmp/empty");
        let out = with_scope_prefix("   ", &work, None);
        assert!(out.contains("CCO scope lock: work ONLY inside `/tmp/empty`"));
        assert!(!out.contains("\n\n"));
    }

    #[test]
    fn validate_task_rejects_bg_does_not_fake_allowed_tools() {
        let p = CodexProvider::new("codex", vec![]);
        let mut task = TaskIR {
            id: "t1".into(),
            title: "t1".into(),
            depends_on: vec![],
            group: None,
            provider: "codex".into(),
            mode: "bg".into(),
            prompt: "hello".into(),
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
        };
        let err = p.validate_task(&task).unwrap_err().to_string();
        assert!(
            err.contains("does not support mode=bg") || err.contains("bg"),
            "expected bg rejection: {err}"
        );
        task.mode = "print".into();
        assert!(p.validate_task(&task).is_ok());
        let caps = p.capabilities();
        assert!(caps.print);
        assert!(!caps.background, "must not fake Claude bg");
        assert!(!caps.session_resume);
    }
}
