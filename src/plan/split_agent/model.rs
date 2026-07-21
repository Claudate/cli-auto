//! ModelSplitAgent — call model / fixture → cco-split/v1.
//!
//! [INPUT]: SplitRequest · Config · env fixtures
//! [OUTPUT]: CcoSplitJob
//! [POS]: plan/split_agent — 实现 ports::SplitAgentPort
//! [PROTOCOL]: 优先 fixture/env → Messages HTTP（有 key）→ Claude CLI print；不启动业务 Worker

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::config::Config;
use crate::domain::plan::CcoSplitJob;
use crate::ports::split_agent::{SplitAgentPort, SplitRequest};
use crate::runtime::provider::sdk_http::{
    resolve_api_key, resolve_base_url, resolve_model, MessagesHttpClient, ReqwestMessagesClient,
};

use super::parse::parse_agent_output;
use super::prompt::{system_prompt, user_prompt};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const SPLIT_MAX_TOKENS: u32 = 8192;
const SPLIT_HTTP_TIMEOUT_SECS: u64 = 180;

/// Production model-backed split agent.
pub struct ModelSplitAgent<'a> {
    pub config: &'a Config,
}

impl<'a> ModelSplitAgent<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }
}

impl SplitAgentPort for ModelSplitAgent<'_> {
    fn split(&self, req: &SplitRequest) -> Result<CcoSplitJob> {
        let raw = obtain_raw_text(self.config, req)?;
        parse_agent_output(&raw, req)
    }
}

/// Fixture agent for tests (no network / CLI).
pub struct FixtureSplitAgent {
    pub raw: String,
}

impl SplitAgentPort for FixtureSplitAgent {
    fn split(&self, req: &SplitRequest) -> Result<CcoSplitJob> {
        parse_agent_output(&self.raw, req)
    }
}

fn obtain_raw_text(config: &Config, req: &SplitRequest) -> Result<String> {
    // 1) Inline env JSON (integration / CI without network)
    if let Ok(v) = std::env::var("CCO_SPLIT_AGENT_JSON") {
        let t = v.trim();
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    // 2) Path to fixture file
    if let Ok(p) = std::env::var("CCO_SPLIT_AGENT_FIXTURE") {
        let t = p.trim();
        if !t.is_empty() {
            return std::fs::read_to_string(t)
                .with_context(|| format!("read CCO_SPLIT_AGENT_FIXTURE {t}"));
        }
    }
    // 3) Messages HTTP when API key present
    if resolve_api_key().is_some() {
        match call_messages_http(req) {
            Ok(s) if !s.trim().is_empty() => return Ok(s),
            Ok(_) => {}
            Err(e) => {
                // Fall through to CLI; caller logs if both fail.
                let _ = e;
            }
        }
    }
    // 4) Claude CLI print (existing planner worker pattern, split prompt)
    call_claude_cli_print(config, req)
}

fn call_messages_http(req: &SplitRequest) -> Result<String> {
    let api_key = resolve_api_key().context("no Anthropic API key")?;
    let model = resolve_model(&[]);
    let base = resolve_base_url();
    let url = format!("{}/v1/messages", base.trim_end_matches('/'));
    let project_label = req
        .project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let plan_md = truncate_plan(&req.plan_md, 40_000);
    let user = user_prompt(project_label, req.max_parallel, &plan_md);
    let body = json!({
        "model": model,
        "max_tokens": SPLIT_MAX_TOKENS,
        "system": system_prompt(),
        "messages": [{"role": "user", "content": user}],
    });
    let headers = [
        ("x-api-key", api_key),
        ("anthropic-version", ANTHROPIC_VERSION.to_string()),
        ("content-type", "application/json".into()),
    ];
    let timeout = Duration::from_secs(
        std::env::var("CCO_SDK_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SPLIT_HTTP_TIMEOUT_SECS),
    );
    let client = ReqwestMessagesClient::new(timeout)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for split agent messages")?;
    let (status, text) = rt.block_on(async {
        client
            .post_json(&url, &headers, body)
            .await
            .context("split agent messages HTTP")
    })?;
    if !(200..300).contains(&status) {
        bail!(
            "Messages HTTP {status}: {}",
            text.chars().take(240).collect::<String>()
        );
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&text).context("parse messages response JSON")?;
    extract_messages_text(&parsed).context("messages response missing text content")
}

fn extract_messages_text(parsed: &serde_json::Value) -> Option<String> {
    let content = parsed.get("content")?.as_array()?;
    let mut parts = Vec::new();
    for block in content {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn call_claude_cli_print(config: &Config, req: &SplitRequest) -> Result<String> {
    use crate::plan::{TaskIR, PLANNER_MAX_BUDGET_USD};
    use crate::runtime::provider::{
        claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
    };

    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = crate::plan::planner::job_dir(config, &req.job_id).join("llm_work");
    let task_dir = work.join("tasks").join("__split_agent__");
    std::fs::create_dir_all(&task_dir)?;
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");

    let project_label = req
        .project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let plan_md = truncate_plan(&req.plan_md, 40_000);
    let user = user_prompt(project_label, req.max_parallel, &plan_md);
    // CLI print: single user-ish prompt with system rules inlined (no separate system role).
    let prompt = format!("{}\n\n{}", system_prompt(), user);
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;

    let planner_task = TaskIR {
        id: "__split_agent__".into(),
        title: "plan split agent".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        acceptance: None,
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: json!({
            "max_turns": 6,
            "max_budget_usd": PLANNER_MAX_BUDGET_USD,
            "permission_mode": "dontAsk",
            "allowed_tools": [],
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
    };

    let ctx = StartCtx {
        run_id: req.job_id.clone(),
        project_root: req.project.clone(),
        work_dir: req.project.clone(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for split agent CLI")?;

    let stdout = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&planner_task)?;
        let handle = provider.start(&planner_task, &ctx).await?;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => break,
            }
        }
        let result = provider.collect(&handle).await?;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result
                .error
                .unwrap_or_else(|| "split agent worker failed".into());
            bail!("split agent CLI not done: {err}\n{}", truncate_plan(&stdout, 500));
        }
        Ok::<String, anyhow::Error>(stdout)
    })?;
    Ok(stdout)
}

fn truncate_plan(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!(
            "{}…\n\n[truncated, {} bytes total]",
            &s[..max],
            s.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fixture_agent_builds_job() {
        let raw = r#"{"schema":"cco-split/v1","title":"T","tasks":[
          {"id":"t1","title":"写入口","body":"实现 main","depends_on":[],"kind":"do"},
          {"id":"t2","title":"补测","body":"单测","depends_on":["t1"],"kind":"check"}
        ]}"#;
        let agent = FixtureSplitAgent {
            raw: raw.to_string(),
        };
        let req = SplitRequest {
            job_id: "j".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("p.md"),
            plan_abs: PathBuf::from("/p/p.md"),
            plan_md: "# p".into(),
            max_parallel: 2,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        };
        let job = agent.split(&req).unwrap();
        assert_eq!(job.tasks.len(), 2);
        assert_eq!(job.tasks[1].depends_on, vec!["t1".to_string()]);
    }
}
