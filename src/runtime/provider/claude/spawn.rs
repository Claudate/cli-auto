//! Claude spawn: print/bg process start + stream stdout.
//!
//! [INPUT]: TaskIR · StartCtx · bin/flags
//! [OUTPUT]: WorkerHandle · log files · stream_child exit code
//! [POS]: claude provider 启动路径；D4 自 claude.rs 抽出；P2-1 拼 scope + append_system_prompt(role=inspect)
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: print 仅 stdin 传 prompt；allowed_tools:[] → --allowedTools ""
//! note: max_turns/max_budget_usd null|0 → 不传 CLI 限制（chat 无人为 turn 上限）
//! note: append-system-prompt = project scope lock + TaskScope contract + provider_opts.append_system_prompt
//! note: permission_mode=bypassPermissions → 另加 --allow-dangerously-skip-permissions（chat 无权限 UI）

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::{Child, Command};
use tracing::{info, warn};

use super::super::{StartCtx, WorkerHandle};
use super::parse_result::parse_agent_id;
use super::ClaudeProvider;
use crate::plan::{TaskIR, TaskScope};

impl ClaudeProvider {
    pub fn new(bin: String, extra_args: Vec<String>) -> Self {
        Self { bin, extra_args }
    }

    pub(super) fn opt_str(opts: &serde_json::Value, key: &str) -> Option<String> {
        opts.get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    /// Map `provider_opts.allowed_tools` → CLI flag value.
    ///
    /// - key **absent** → `None` (do not pass `--allowedTools`; CLI keeps defaults)
    /// - **empty array** `[]` → `Some("")` (explicitly no tools — chat/planner text-only)
    /// - non-empty array / string → comma-joined allowlist
    ///
    /// Historical bug: empty array returned `None`, so chat/planner silently got **all** tools.
    pub(super) fn opt_tools(opts: &serde_json::Value) -> Option<String> {
        let v = opts.get("allowed_tools")?;
        if let Some(arr) = v.as_array() {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            // Empty list is intentional "no tools", not "omit flag".
            Some(parts.join(","))
        } else {
            v.as_str().map(|s| s.to_string())
        }
    }

    /// Whether to pass `--max-turns` / `--max-budget-usd`.
    ///
    /// - key **absent** → `Some(default)` (worker safety net)
    /// - key **null** or **0** → `None` (omit flag — chat has no artificial turn/budget cap)
    /// - key number → `Some(n)`
    pub(super) fn opt_limit_u32(opts: &serde_json::Value, key: &str, default: u32) -> Option<u32> {
        match opts.get(key) {
            None => Some(default),
            Some(v) if v.is_null() => None,
            Some(v) => {
                let n = v
                    .as_u64()
                    .map(|n| n as u32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()));
                match n {
                    Some(0) | None => None,
                    Some(n) => Some(n),
                }
            }
        }
    }

    pub(super) fn opt_limit_f64(opts: &serde_json::Value, key: &str, default: f64) -> Option<f64> {
        match opts.get(key) {
            None => Some(default),
            Some(v) if v.is_null() => None,
            Some(v) => {
                let n = v
                    .as_f64()
                    .or_else(|| v.as_i64().map(|i| i as f64))
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()));
                match n {
                    Some(x) if x <= 0.0 => None,
                    Some(x) => Some(x),
                    None => None,
                }
            }
        }
    }

    /// Build `--append-system-prompt` body: project lock + optional TaskScope + opts segment.
    ///
    /// Pure helper for unit tests. `provider_opts.append_system_prompt` already carries
    /// P2-1 `role=inspect` text after [`crate::plan::materialize_role_defaults`].
    pub(super) fn build_append_system_prompt(
        work_dir: &Path,
        task_scope: Option<&TaskScope>,
        opts: &serde_json::Value,
    ) -> String {
        let mut parts = vec![format!(
            "CCO scope lock: work ONLY inside `{dir}`. Never read, list, search, or write outside this project directory. FORBIDDEN: home (~), Desktop, Documents, Downloads, Pictures, Movies, Music, Photos, and any absolute path not under `{dir}`. Do NOT run `find ~`, `ls ~`, `find /Users`, or any home-wide scan. Prefer relative paths from cwd.",
            dir = work_dir.display()
        )];

        if let Some(s) = task_scope {
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

        let extra_sys = Self::opt_str(opts, "append_system_prompt").unwrap_or_default();
        if !extra_sys.trim().is_empty() {
            parts.push(extra_sys);
        }
        // Ultracode: multi-agent thoroughness (product token; CLI flag is still xhigh).
        if let Some(raw) = Self::opt_str(opts, "effort") {
            if crate::config::effort_is_ultracode(&raw) {
                parts.push(crate::config::ULTRACODE_SYSTEM_HINT.to_string());
            }
        } else if opts
            .get("effort")
            .and_then(|v| v.as_str())
            .map(crate::config::effort_is_ultracode)
            .unwrap_or(false)
        {
            parts.push(crate::config::ULTRACODE_SYSTEM_HINT.to_string());
        }
        parts.join("\n\n")
    }

    pub(super) fn apply_common_flags(
        &self,
        cmd: &mut Command,
        opts: &serde_json::Value,
        for_print: bool,
        work_dir: &Path,
        task_scope: Option<&TaskScope>,
    ) {
        // Chat passes null → omit flags (no artificial turn/budget ceiling).
        // Workers omit the key → defaults 40 turns / $10.
        let max_turns = Self::opt_limit_u32(opts, "max_turns", 40);
        let max_budget = Self::opt_limit_f64(opts, "max_budget_usd", 10.0);
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
                .arg("--verbose");
            if let Some(n) = max_turns {
                cmd.arg("--max-turns").arg(n.to_string());
            }
            if let Some(b) = max_budget {
                cmd.arg("--max-budget-usd").arg(b.to_string());
            }
        } else {
            // bg: still pass permission / tools / model when supported
            cmd.arg("--bg");
        }
        cmd.arg("--permission-mode").arg(&perm);
        // Non-interactive desktop/print sessions: bypass mode needs the allow flag,
        // otherwise CLI may still refuse to enter bypassPermissions.
        if perm == "bypassPermissions" {
            cmd.arg("--allow-dangerously-skip-permissions");
        }
        // Always pass when key present (including empty string = no tools).
        if let Some(t) = tools {
            cmd.arg("--allowedTools").arg(t);
        }
        if let Some(m) = model {
            if !m.is_empty() {
                cmd.arg("--model").arg(m);
            }
        }
        // Reasoning effort: low|medium|high|xhigh|max|ultracode → claude --effort
        // ultracode maps to xhigh on the flag; thoroughness hint via system prompt.
        // Missing key → high (product default); null/empty → omit flag.
        let effort_raw = Self::opt_str(opts, "effort");
        let effort_norm = effort_raw
            .as_deref()
            .and_then(crate::config::normalize_effort)
            .or_else(|| {
                // Key absent → default high; explicit empty/null → leave unset
                if opts.get("effort").is_none() {
                    Some("high".into())
                } else {
                    None
                }
            });
        if let Some(norm) = effort_norm {
            let level = crate::config::effort_cli_level(&norm);
            cmd.arg("--effort").arg(level);
        }
        // 项目范围锁 + TaskScope + role segment (inspect via materialize_role_defaults).
        // 子进程挂在 CCO.app 身份下，home 扫描会触发 macOS TCC 授权弹窗。
        let sys = Self::build_append_system_prompt(work_dir, task_scope, opts);
        cmd.arg("--append-system-prompt").arg(sys);
        for a in &self.extra_args {
            cmd.arg(a);
        }
    }

    pub(super) fn apply_env(cmd: &mut Command, ctx: &StartCtx) {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            cmd.env("ANTHROPIC_API_KEY", key);
        }
        // GUI/.app: inject Homebrew/user bins so sibling tools resolve.
        super::super::apply_worker_process_env(cmd, &ctx.env_extra);
    }

    pub(super) async fn start_print(
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
        self.apply_common_flags(
            &mut cmd,
            &task.provider_opts,
            true,
            &ctx.work_dir,
            task.scope.as_ref(),
        );
        // Print prompt via **stdin only**. Claude -p rejects / misbehaves when both a
        // positional prompt and stdin are provided ("stdin or prompt argument").
        // Multline + CJK history (chat) also blows past safe argv sizes if doubled.
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
        } else {
            bail!("claude print spawn: stdin not piped");
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
                    // Preserve stop_run's `.done=130`; map SIGKILL (-1) → 130.
                    let code = crate::runtime::provider::finalize_stream_exit(&done_flag, code);
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
                    // Do not clobber orchestrator stop marker with -1.
                    let code = crate::runtime::provider::finalize_stream_exit(&done_flag, -1);
                    let meta = serde_json::json!({
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

    pub(super) async fn start_bg(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: PathBuf,
        stderr_path: PathBuf,
        meta_path: PathBuf,
    ) -> Result<WorkerHandle> {
        let mut cmd = Command::new(&self.bin);
        self.apply_common_flags(
            &mut cmd,
            &task.provider_opts,
            false,
            &ctx.work_dir,
            task.scope.as_ref(),
        );
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
}

/// Pump process stdout/stderr into log files while the child runs.
/// Returns the process exit code (124 on timeout).
pub(super) async fn stream_child(
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

#[cfg(test)]
mod tests {
    use super::ClaudeProvider;
    use crate::plan::TaskScope;
    use std::path::Path;

    #[test]
    fn opt_limit_u32_null_or_zero_omits_flag() {
        let absent = serde_json::json!({});
        assert_eq!(ClaudeProvider::opt_limit_u32(&absent, "max_turns", 40), Some(40));

        let null = serde_json::json!({ "max_turns": null });
        assert_eq!(ClaudeProvider::opt_limit_u32(&null, "max_turns", 40), None);

        let zero = serde_json::json!({ "max_turns": 0 });
        assert_eq!(ClaudeProvider::opt_limit_u32(&zero, "max_turns", 40), None);

        let set = serde_json::json!({ "max_turns": 8 });
        assert_eq!(ClaudeProvider::opt_limit_u32(&set, "max_turns", 40), Some(8));
    }

    #[test]
    fn opt_limit_f64_null_or_zero_omits_flag() {
        let absent = serde_json::json!({});
        assert_eq!(
            ClaudeProvider::opt_limit_f64(&absent, "max_budget_usd", 10.0),
            Some(10.0)
        );
        let null = serde_json::json!({ "max_budget_usd": null });
        assert_eq!(
            ClaudeProvider::opt_limit_f64(&null, "max_budget_usd", 10.0),
            None
        );
        let set = serde_json::json!({ "max_budget_usd": 3.5 });
        assert_eq!(
            ClaudeProvider::opt_limit_f64(&set, "max_budget_usd", 10.0),
            Some(3.5)
        );
    }

    #[test]
    fn effort_cli_mapping_and_ultracode_hint() {
        assert_eq!(crate::config::effort_cli_level("low"), "low");
        assert_eq!(crate::config::effort_cli_level("xhigh"), "xhigh");
        assert_eq!(crate::config::effort_cli_level("ultracode"), "xhigh");
        assert!(crate::config::effort_is_ultracode("ultracode"));
        assert!(!crate::config::effort_is_ultracode("high"));

        // Ultracode injects thoroughness hint into append system prompt.
        let opts = serde_json::json!({ "effort": "ultracode" });
        let sys = ClaudeProvider::build_append_system_prompt(
            Path::new("/tmp/proj"),
            None::<&TaskScope>,
            &opts,
        );
        assert!(
            sys.contains("Ultracode is on"),
            "expected ultracode hint, got: {sys}"
        );
        let opts_high = serde_json::json!({ "effort": "high" });
        let sys_high = ClaudeProvider::build_append_system_prompt(
            Path::new("/tmp/proj"),
            None::<&TaskScope>,
            &opts_high,
        );
        assert!(!sys_high.contains("Ultracode is on"));
    }

    /// P2-1: inspect role segment (from materialize) + scope.paths land in system prompt.
    #[test]
    fn build_append_system_prompt_includes_scope_and_inspect_segment() {
        let work = Path::new("/tmp/proj");
        let scope = TaskScope {
            paths: vec![".cco-out/inspect/**".into()],
            readonly: vec!["src/**".into()],
            forbid: vec![],
        };
        let opts = serde_json::json!({
            "append_system_prompt": "CCO role=inspect: terminal quality gate, not an implementer. Business tree is READ-ONLY."
        });
        let sys = ClaudeProvider::build_append_system_prompt(work, Some(&scope), &opts);
        assert!(
            sys.contains("CCO scope lock: work ONLY inside `/tmp/proj`"),
            "{sys}"
        );
        assert!(
            sys.contains("Writable whitelist (scope.paths): .cco-out/inspect/**"),
            "{sys}"
        );
        assert!(sys.contains("Extra readonly ranges (scope.readonly): src/**"), "{sys}");
        assert!(sys.contains("CCO role=inspect:"), "{sys}");
        assert!(sys.contains("READ-ONLY"), "{sys}");
    }

    #[test]
    fn build_append_system_prompt_without_scope_is_project_lock_only() {
        let work = Path::new("/tmp/empty");
        let opts = serde_json::json!({});
        let sys = ClaudeProvider::build_append_system_prompt(work, None, &opts);
        assert!(sys.contains("CCO scope lock:"));
        assert!(!sys.contains("Writable whitelist"));
        assert!(!sys.contains("CCO role=inspect:"));
    }
}

