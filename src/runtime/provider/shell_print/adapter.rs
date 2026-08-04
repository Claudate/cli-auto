//! Generic shell-print WorkerPort for codex-like / gemini-like CLIs.
//!
//! [INPUT]: ShellProfile · bin · extra_args · TaskIR · StartCtx
//! [OUTPUT]: spawn/poll/stop/collect
//! [POS]: runtime/provider/shell_print
//! [PROTOCOL]: start 必须 remove `.done`；禁止 spawn 时网络安装

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::{Child, Command};
use tracing::{info, warn};

use super::profiles::{PromptPlacement, ShellProfile};
use super::scope::with_scope_prefix;
use super::stream::{process_alive, stop_pid, stream_child};
use crate::plan::TaskIR;
use crate::ports::worker::{
    Capabilities, StartCtx, TaskResult, TaskStatus, WorkerHandle, WorkerPort, WorkerStatus,
};
use crate::runtime::provider::{
    ensure_done_marker, finalize_stream_exit, parse_claude_result_json, resolve_exit_code,
    task_status_from_exit, worker_status_from_exit,
};

pub struct ShellPrintProvider {
    profile: ShellProfile,
    bin: String,
    extra_args: Vec<String>,
}

impl ShellPrintProvider {
    pub fn new(profile: ShellProfile, bin: impl Into<String>, extra_args: Vec<String>) -> Self {
        Self {
            profile,
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
        let full_auto = Self::opt_bool(opts, "full_auto").unwrap_or(true);
        let json = Self::opt_bool(opts, "json").unwrap_or(true);
        let model = Self::opt_str(opts, "model");

        if full_auto {
            for a in self.profile.yolo_args {
                cmd.arg(a);
            }
        }
        if json {
            for a in self.profile.json_args {
                cmd.arg(a);
            }
        }
        if let (Some(flag), Some(m)) = (self.profile.model_flag, model) {
            cmd.arg(flag).arg(m);
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

    fn build_command(&self, prompt: &str, opts: &serde_json::Value) -> Command {
        let mut cmd = Command::new(&self.bin);
        match self.profile.placement {
            PromptPlacement::SubcommandThenTrailing => {
                if let Some(sub) = self.profile.subcommand {
                    cmd.arg(sub);
                }
                self.apply_common_flags(&mut cmd, opts);
                cmd.arg(prompt);
            }
            PromptPlacement::FlagOrTrailing => {
                self.apply_common_flags(&mut cmd, opts);
                if let Some(flag) = self.profile.prompt_flag {
                    cmd.arg(flag).arg(prompt);
                } else {
                    cmd.arg(prompt);
                }
            }
        }
        cmd
    }

    async fn start_exec(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        let prompt = with_scope_prefix(&task.prompt, &ctx.work_dir, task.scope.as_ref());
        let _ = std::fs::write(&stdout_path, "");
        let _ = std::fs::write(
            &stderr_path,
            format!(
                "[{}] starting {} · task={} · cwd={}\n",
                chrono::Utc::now().to_rfc3339(),
                self.profile.name,
                task.id,
                ctx.work_dir.display()
            ),
        );

        let mut cmd = self.build_command(&prompt, &task.provider_opts);
        cmd.current_dir(&ctx.work_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        crate::runtime::provider::apply_worker_process_env(&mut cmd, &ctx.env_extra);

        info!(
            task = %task.id,
            provider = self.profile.name,
            bin = %self.bin,
            cwd = %ctx.work_dir.display(),
            "starting shell-print worker"
        );

        let child: Child = cmd
            .spawn()
            .with_context(|| format!("spawn {} ({})", self.bin, self.profile.install_hint))?;
        let pid = child.id();
        let opaque = pid
            .map(|p| format!("pid:{p}"))
            .unwrap_or_else(|| format!("{}:{}", self.profile.name, task.id));

        let timeout_secs = task
            .provider_opts
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60 * 30);
        let timeout = Duration::from_secs(timeout_secs);
        let provider_name = self.profile.name.to_string();

        let handle = WorkerHandle {
            provider: provider_name.clone(),
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
        let provider_meta = provider_name.clone();

        tokio::spawn(async move {
            let result = stream_child(child, timeout, &stdout_path_c, &stderr_path_c).await;
            match result {
                Ok(code) => {
                    let code = finalize_stream_exit(&done_flag, code);
                    let meta = serde_json::json!({
                        "provider": provider_meta,
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
                    let code = finalize_stream_exit(&done_flag, -1);
                    let meta = serde_json::json!({
                        "provider": provider_meta,
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
            "provider": provider_name,
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
impl WorkerPort for ShellPrintProvider {
    fn name(&self) -> &str {
        self.profile.name
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
                "{} binary not found: {} ({})",
                self.profile.name, self.bin, self.profile.install_hint
            )
        })?;
        let path_env = crate::runtime::provider::worker_path_env();
        let out = Command::new(&path)
            .args(self.profile.version_args)
            .env("PATH", &path_env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                if let Some(alt) = self.profile.alt_version_args {
                    let out2 = Command::new(&path)
                        .args(alt)
                        .env("PATH", &path_env)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .await;
                    if out2.map(|x| x.status.success()).unwrap_or(false) {
                        return Ok(());
                    }
                }
                if path.exists() {
                    warn!(
                        bin = %self.bin,
                        provider = self.profile.name,
                        stderr = %err,
                        "version probe weak; binary exists"
                    );
                    Ok(())
                } else {
                    bail!(
                        "{} preflight failed: {err} ({})",
                        self.profile.name,
                        self.profile.install_hint
                    );
                }
            }
            Err(e) => bail!(
                "{} preflight spawn failed: {e} ({})",
                self.profile.name,
                self.profile.install_hint
            ),
        }
    }

    fn validate_task(&self, task: &TaskIR) -> Result<()> {
        if task.prompt.trim().is_empty() {
            bail!("{} task requires non-empty prompt", self.profile.name);
        }
        if task.mode.eq_ignore_ascii_case("bg") {
            bail!(
                "{} provider does not support mode=bg yet (use print/exec)",
                self.profile.name
            );
        }
        Ok(())
    }

    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle> {
        std::fs::create_dir_all(&ctx.task_dir)?;
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
            return Ok(worker_status_from_exit(code));
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
            stop_pid(pid).await;
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
        let exit_code = resolve_exit_code(meta_code, done_code);

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
            other => task_status_from_exit(other),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::shell_print::profiles::CODEX;

    #[test]
    fn validate_rejects_bg() {
        let p = ShellPrintProvider::new(CODEX, "codex", vec![]);
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
        assert!(err.contains("bg"), "expected bg rejection: {err}");
        task.mode = "print".into();
        assert!(p.validate_task(&task).is_ok());
        let caps = p.capabilities();
        assert!(caps.print);
        assert!(!caps.background);
        assert!(!caps.session_resume);
    }
}
