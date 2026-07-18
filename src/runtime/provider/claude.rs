//! Claude CLI provider: print (`-p`) and background (`--bg`).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use regex::Regex;
use tokio::process::{Child, Command};
use tracing::{info, warn};

use super::{
    ensure_done_marker, parse_claude_result_json, Capabilities, StartCtx, TaskResult, TaskStatus,
    WorkerHandle, WorkerProvider, WorkerStatus,
};
use crate::plan::TaskIR;

pub struct ClaudeProvider {
    bin: String,
    extra_args: Vec<String>,
}

impl ClaudeProvider {
    pub fn new(bin: String, extra_args: Vec<String>) -> Self {
        Self { bin, extra_args }
    }

    fn opt_u32(opts: &serde_json::Value, key: &str) -> Option<u32> {
        opts.get(key).and_then(|v| {
            v.as_u64()
                .map(|n| n as u32)
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
    }

    fn opt_f64(opts: &serde_json::Value, key: &str) -> Option<f64> {
        opts.get(key).and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_i64().map(|i| i as f64))
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
    }

    fn opt_str(opts: &serde_json::Value, key: &str) -> Option<String> {
        opts.get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    fn opt_tools(opts: &serde_json::Value) -> Option<String> {
        let v = opts.get("allowed_tools")?;
        if let Some(arr) = v.as_array() {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(","))
            }
        } else {
            v.as_str().map(|s| s.to_string())
        }
    }

    fn apply_common_flags(
        &self,
        cmd: &mut Command,
        opts: &serde_json::Value,
        for_print: bool,
        work_dir: &Path,
    ) {
        let max_turns = Self::opt_u32(opts, "max_turns").unwrap_or(40);
        let max_budget = Self::opt_f64(opts, "max_budget_usd").unwrap_or(10.0);
        let perm = Self::opt_str(opts, "permission_mode").unwrap_or_else(|| "dontAsk".into());
        let tools = Self::opt_tools(opts);
        let model = Self::opt_str(opts, "model");

        if for_print {
            // stream-json emits progressive NDJSON so the desktop can show live CLI output.
            // collect() still parses the final result object via parse_claude_result_json.
            cmd.arg("-p")
                .arg("--bare")
                .arg("--output-format")
                .arg("stream-json")
                .arg("--verbose")
                .arg("--max-turns")
                .arg(max_turns.to_string())
                .arg("--max-budget-usd")
                .arg(max_budget.to_string());
        } else {
            // bg: still pass permission / tools / model when supported
            cmd.arg("--bg");
        }
        cmd.arg("--permission-mode").arg(&perm);
        if let Some(t) = tools {
            cmd.arg("--allowedTools").arg(t);
        }
        if let Some(m) = model {
            if !m.is_empty() {
                cmd.arg("--model").arg(m);
            }
        }
        // 项目范围锁：子进程挂在 CCO.app 身份下，home 扫描会触发 macOS
        // 对 Desktop/Documents/Downloads/Photos/Music 的 TCC 授权弹窗。
        // 用 append-system-prompt 约束 agent 只在 work_dir 内活动。
        let scope = format!(
            "CCO scope lock: work ONLY inside `{dir}`. Never read, list, search, or write outside this project directory. FORBIDDEN: home (~), Desktop, Documents, Downloads, Pictures, Movies, Music, Photos, and any absolute path not under `{dir}`. Do NOT run `find ~`, `ls ~`, `find /Users`, or any home-wide scan. Prefer relative paths from cwd.",
            dir = work_dir.display()
        );
        let extra_sys = Self::opt_str(opts, "append_system_prompt").unwrap_or_default();
        let sys = if extra_sys.trim().is_empty() {
            scope
        } else {
            format!("{scope}

{extra_sys}")
        };
        cmd.arg("--append-system-prompt").arg(sys);
        for a in &self.extra_args {
            cmd.arg(a);
        }
    }

    fn apply_env(cmd: &mut Command, ctx: &StartCtx) {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            cmd.env("ANTHROPIC_API_KEY", key);
        }
        for (k, v) in &ctx.env_extra {
            cmd.env(k, v);
        }
    }

    async fn start_print(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        // Prefer stdin for the user prompt: more reliable with multiline / unicode
        // than trailing argv (Claude 2.x: "stdin or prompt argument when using --print").
        let prompt = task.prompt.clone();
        let prompt_file = ctx.task_dir.join("prompt.md");
        std::fs::write(&prompt_file, &prompt)?;

        // Seed log files so the UI shows activity immediately while the process starts.
        let _ = std::fs::write(&stdout_path, "");
        let _ = std::fs::write(
            &stderr_path,
            format!(
                "[{}] starting claude print · task={} · cwd={}\n",
                chrono::Utc::now().to_rfc3339(),
                task.id,
                ctx.work_dir.display()
            ),
        );

        let mut cmd = Command::new(&self.bin);
        self.apply_common_flags(&mut cmd, &task.provider_opts, true, &ctx.work_dir);
        // Also pass as positional for CLIs that prefer argv; stdin is the primary path.
        cmd.arg(&prompt);
        cmd.current_dir(&ctx.work_dir);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);
        Self::apply_env(&mut cmd, ctx);

        info!(task = %task.id, bin = %self.bin, cwd = %ctx.work_dir.display(), "starting claude print");

        let mut child: Child = cmd.spawn().with_context(|| format!("spawn {}", self.bin))?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
                warn!(error = %e, "failed writing prompt to claude stdin");
            }
            let _ = stdin.shutdown().await;
        }
        let pid = child.id();
        let opaque = format!("pid:{}", pid.unwrap_or(0));
        let handle = WorkerHandle {
            provider: "claude".into(),
            task_id: task.id.clone(),
            mode: "print".into(),
            opaque_id: opaque.clone(),
            pid,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        let done_flag = ctx.task_dir.join(".done");
        let timeout = task
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(3600));
        let stdout_path_c = stdout_path.clone();
        let stderr_path_c = stderr_path.clone();
        let meta_path_c = meta_path.clone();

        // Stream stdout/stderr to disk as the process runs so the desktop UI can
        // poll live log tails instead of waiting until exit.
        tokio::spawn(async move {
            let result = stream_child(child, timeout, &stdout_path_c, &stderr_path_c).await;
            match result {
                Ok(code) => {
                    let meta = serde_json::json!({
                        "exit_code": code,
                        "pid": pid,
                        "mode": "print",
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
            "provider": "claude",
            "mode": "print",
            "pid": pid,
            "opaque_id": opaque,
            "started_at": handle.started_at.to_rfc3339(),
            "prompt_bytes": prompt.len(),
        });
        std::fs::write(&meta_path, serde_json::to_string_pretty(&start_meta)?)?;
        Ok(handle)
    }

    async fn start_bg(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        let mut cmd = Command::new(&self.bin);
        self.apply_common_flags(&mut cmd, &task.provider_opts, false, &ctx.work_dir);
        // name-like identity via prompt prefix is not a flag; pass prompt as arg
        cmd.arg(&task.prompt);
        cmd.current_dir(&ctx.work_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        Self::apply_env(&mut cmd, ctx);

        info!(task = %task.id, bin = %self.bin, cwd = %ctx.work_dir.display(), "starting claude --bg");

        let output = tokio::time::timeout(Duration::from_secs(60), cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("claude --bg spawn timeout"))?
            .with_context(|| format!("spawn {} --bg", self.bin))?;

        let out = String::from_utf8_lossy(&output.stdout).to_string();
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = std::fs::write(&stderr_path, &err);
        let _ = std::fs::write(ctx.task_dir.join("bg_spawn.stdout"), &out);

        if !output.status.success() && parse_agent_id(&out).is_none() && parse_agent_id(&err).is_none()
        {
            bail!(
                "claude --bg failed: {} {}",
                err.chars().take(400).collect::<String>(),
                out.chars().take(200).collect::<String>()
            );
        }

        let agent_id = parse_agent_id(&out)
            .or_else(|| parse_agent_id(&err))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "could not parse agent id from claude --bg output: {}",
                    out.chars().take(300).collect::<String>()
                )
            })?;

        // timeout tracker
        let deadline = task
            .timeout_secs
            .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s as i64));

        let handle = WorkerHandle {
            provider: "claude".into(),
            task_id: task.id.clone(),
            mode: "bg".into(),
            opaque_id: format!("agent:{agent_id}"),
            pid: None,
            started_at: chrono::Utc::now(),
            stdout_path: stdout_path.clone(),
            meta_path: meta_path.clone(),
        };

        let start_meta = serde_json::json!({
            "provider": "claude",
            "mode": "bg",
            "agent_id": agent_id,
            "opaque_id": handle.opaque_id,
            "started_at": handle.started_at.to_rfc3339(),
            "deadline": deadline.map(|d| d.to_rfc3339()),
            "work_dir": ctx.work_dir,
        });
        std::fs::write(&meta_path, serde_json::to_string_pretty(&start_meta)?)?;

        // initial empty logs file
        if !stdout_path.exists() {
            let _ = std::fs::write(&stdout_path, "");
        }

        Ok(handle)
    }

    async fn agents_json(&self) -> Result<serde_json::Value> {
        let out = Command::new(&self.bin)
            .args(["agents", "--json", "--all"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("claude agents --json --all")?;
        let text = String::from_utf8_lossy(&out.stdout);
        if text.trim().is_empty() {
            // try without --all
            let out2 = Command::new(&self.bin)
                .args(["agents", "--json"])
                .output()
                .await
                .context("claude agents --json")?;
            let text2 = String::from_utf8_lossy(&out2.stdout);
            return parse_json_lenient(&text2);
        }
        parse_json_lenient(&text)
    }

    fn agent_id_from_handle(handle: &WorkerHandle) -> Option<String> {
        handle
            .opaque_id
            .strip_prefix("agent:")
            .map(|s| s.to_string())
            .or_else(|| {
                let meta = std::fs::read_to_string(&handle.meta_path).ok()?;
                let v: serde_json::Value = serde_json::from_str(&meta).ok()?;
                v.get("agent_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
    }

    async fn refresh_bg_logs(&self, handle: &WorkerHandle, agent_id: &str) {
        let out = Command::new(&self.bin)
            .args(["logs", agent_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        if let Ok(o) = out {
            let text = String::from_utf8_lossy(&o.stdout);
            if !text.is_empty() {
                let _ = std::fs::write(&handle.stdout_path, text.as_ref());
            }
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.is_empty() {
                if let Some(parent) = handle.stdout_path.parent() {
                    let _ = std::fs::write(parent.join("stderr.log"), err.as_ref());
                }
            }
        }
    }

    fn bg_deadline_passed(meta: &serde_json::Value) -> bool {
        meta.get("deadline")
            .and_then(|d| d.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| chrono::Utc::now() > d.with_timezone(&chrono::Utc))
            .unwrap_or(false)
    }
}

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
                return Ok(match code {
                    0 => WorkerStatus::Done,
                    124 => WorkerStatus::Timeout,
                    130 => WorkerStatus::Stopped,
                    _ => WorkerStatus::Failed,
                });
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
                    if let Some(st) = find_agent_state(&v, &agent_id) {
                        let norm = st.to_ascii_lowercase();
                        if matches!(
                            norm.as_str(),
                            "done" | "completed" | "complete" | "success" | "finished"
                        ) {
                            let _ = std::fs::write(&done_flag, "0");
                            return Ok(WorkerStatus::Done);
                        }
                        if matches!(
                            norm.as_str(),
                            "failed" | "error" | "crashed" | "dead"
                        ) {
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
                if code == 0 {
                    Ok(WorkerStatus::Done)
                } else if code == 124 {
                    Ok(WorkerStatus::Timeout)
                } else if code == 130 {
                    Ok(WorkerStatus::Stopped)
                } else {
                    Ok(WorkerStatus::Failed)
                }
            } else if let Some(pid) = handle.pid {
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
    }

    async fn stop(&self, handle: &WorkerHandle) -> Result<()> {
        if handle.mode == "bg" {
            if let Some(id) = Self::agent_id_from_handle(handle) {
                let _ = Command::new(&self.bin)
                    .args(["stop", &id])
                    .output()
                    .await;
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

        let status = match exit_code {
            Some(0) => TaskStatus::Done,
            Some(124) => TaskStatus::Timeout,
            Some(130) => TaskStatus::Stopped,
            Some(_) => TaskStatus::Failed,
            None if handle.mode == "bg" && ensure_done_marker(&stdout) => TaskStatus::Done,
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
            agent_id,
            cost_usd: cost,
            raw: parsed.unwrap_or(meta),
            error,
        })
    }
}

/// Parse agent id from `backgrounded · 895cb666 (...)` or similar.
pub fn parse_agent_id(text: &str) -> Option<String> {
    let patterns = [
        r"(?i)backgrounded\s*[·•\-:]?\s*([a-f0-9]{6,})",
        r"(?i)agent\s+id[:\s]+([a-f0-9]{6,})",
        r"(?i)session[:\s]+([a-f0-9]{6,})",
        r"\b([a-f0-9]{8})\b",
    ];
    for p in patterns {
        if let Ok(re) = Regex::new(p) {
            if let Some(c) = re.captures(text) {
                return Some(c[1].to_string());
            }
        }
    }
    None
}

fn parse_json_lenient(text: &str) -> Result<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::json!([]));
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }
    // find array
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    bail!("invalid agents json")
}

fn find_agent_state(v: &serde_json::Value, agent_id: &str) -> Option<String> {
    let arr = if let Some(a) = v.as_array() {
        a.clone()
    } else if let Some(a) = v.get("agents").and_then(|x| x.as_array()) {
        a.clone()
    } else if let Some(a) = v.get("sessions").and_then(|x| x.as_array()) {
        a.clone()
    } else {
        return None;
    };
    for item in arr {
        let id = item
            .get("id")
            .or_else(|| item.get("agent_id"))
            .or_else(|| item.get("session_id"))
            .or_else(|| item.get("short_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        // also match sessionId uuid prefix
        let session_id = item
            .get("sessionId")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let matched = id == agent_id
            || (!id.is_empty() && (id.starts_with(agent_id) || agent_id.starts_with(id)))
            || session_id.starts_with(agent_id)
            || (!session_id.is_empty() && agent_id.len() >= 8 && session_id.contains(agent_id));
        if matched {
            let st = item
                .get("state")
                .or_else(|| item.get("status"))
                .or_else(|| item.get("phase"))
                .and_then(|x| x.as_str())
                .unwrap_or("running");
            return Some(st.to_string());
        }
    }
    None
}

/// Pump process stdout/stderr into log files while the child runs.
/// Returns the process exit code (124 on timeout).
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
    let mut err_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
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

    #[test]
    fn parses_backgrounded_line() {
        let t = "backgrounded · 895cb666 (idle — send a prompt to start)";
        assert_eq!(parse_agent_id(t).as_deref(), Some("895cb666"));
    }

    #[test]
    fn find_agent_state_works() {
        let v = serde_json::json!([
            {"id": "abc12345", "state": "running"},
            {"id": "895cb666", "status": "done"}
        ]);
        assert_eq!(find_agent_state(&v, "895cb666").as_deref(), Some("done"));
    }
}

// silence unused import in some builds
#[allow(dead_code)]
fn _path_ty(_: &Path) {}
