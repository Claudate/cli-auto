//! Plan job: analyze a plan document into a validated PlanIR before exec.
//!
//! Modes:
//! - `parse` — existing adapters (structured / serial-prompts / raw-single)
//! - `fake`  — fixed multi-task DAG for demos without API
//! - `ai`    — heuristic section split (B1 interim); real LLM planner plugs in later

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::topo_layers;
use crate::plan::adapters::raw_single::default_provider_opts;
use crate::plan::{load_plan, OnFailure, PlanIR, TaskIR};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanJobStatus {
    Planning,
    Planned,
    PlanFailed,
    Confirmed,
    Cancelled,
}

impl PlanJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Planned => "planned",
            Self::PlanFailed => "plan_failed",
            Self::Confirmed => "confirmed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanJob {
    pub job_id: String,
    pub status: PlanJobStatus,
    pub project: PathBuf,
    pub plan_path: PathBuf,
    /// parse | fake | ai
    pub plan_mode: String,
    /// worker provider after confirm
    pub provider: String,
    /// worker mode after confirm
    pub exec_mode: String,
    pub error: Option<String>,
    pub run_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Set when planned
    pub plan_name: Option<String>,
    pub task_count: Option<usize>,
    pub max_parallel: Option<usize>,
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanTaskView {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub group: Option<String>,
    pub prompt_preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanJobView {
    pub job_id: String,
    pub status: String,
    pub project: String,
    pub plan_path: String,
    pub plan_mode: String,
    pub provider: String,
    pub exec_mode: String,
    pub error: Option<String>,
    pub run_id: Option<String>,
    pub plan_name: Option<String>,
    pub task_count: Option<usize>,
    pub max_parallel: Option<usize>,
    pub adapter: Option<String>,
    pub layers: Vec<Vec<String>>,
    pub tasks: Vec<PlanTaskView>,
    pub planner_log_tail: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartPlanJobRequest {
    pub project: PathBuf,
    pub plan: PathBuf,
    /// parse | fake | ai  (default: parse)
    pub plan_mode: Option<String>,
    pub provider: Option<String>,
    pub mode: Option<String>,
}

pub fn plan_jobs_dir(config: &Config) -> PathBuf {
    config.state_root.join("plan_jobs")
}

pub fn job_dir(config: &Config, job_id: &str) -> PathBuf {
    plan_jobs_dir(config).join(job_id)
}

fn new_job_id() -> String {
    // Distinct from run ids for clarity in UI/logs.
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let suffix: u32 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        % 900_000)
        + 100_000;
    format!("plan-{ts}-{suffix}")
}

impl PlanJob {
    pub fn save(&self, config: &Config) -> Result<()> {
        let dir = job_dir(config, &self.job_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("job.json");
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn load(config: &Config, job_id: &str) -> Result<Self> {
        let path = job_dir(config, job_id).join("job.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("load plan job {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }
}

fn append_log(config: &Config, job_id: &str, line: &str) {
    let path = job_dir(config, job_id).join("planner.log");
    let _ = std::fs::create_dir_all(job_dir(config, job_id));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{}] {}", Utc::now().to_rfc3339(), line);
    }
}

fn read_log_tail(config: &Config, job_id: &str, max_bytes: usize) -> String {
    let path = job_dir(config, job_id).join("planner.log");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(max_bytes);
            let slice = &bytes[start..];
            String::from_utf8_lossy(slice).into_owned()
        }
        Err(_) => String::new(),
    }
}

fn task_view(t: &TaskIR) -> PlanTaskView {
    let preview: String = t.prompt.chars().take(280).collect();
    PlanTaskView {
        id: t.id.clone(),
        title: t.title.clone(),
        depends_on: t.depends_on.clone(),
        group: t.group.clone(),
        prompt_preview: if t.prompt.chars().count() > 280 {
            format!("{preview}…")
        } else {
            preview
        },
    }
}

pub fn job_view(config: &Config, job: &PlanJob, log_max: usize) -> Result<PlanJobView> {
    let mut layers = Vec::new();
    let mut tasks = Vec::new();
    if matches!(job.status, PlanJobStatus::Planned | PlanJobStatus::Confirmed) {
        if let Ok(ir) = load_proposed(config, &job.job_id) {
            layers = topo_layers(&ir);
            tasks = ir.tasks.iter().map(task_view).collect();
        }
    }
    Ok(PlanJobView {
        job_id: job.job_id.clone(),
        status: job.status.as_str().to_string(),
        project: job.project.display().to_string(),
        plan_path: job.plan_path.display().to_string(),
        plan_mode: job.plan_mode.clone(),
        provider: job.provider.clone(),
        exec_mode: job.exec_mode.clone(),
        error: job.error.clone(),
        run_id: job.run_id.clone(),
        plan_name: job.plan_name.clone(),
        task_count: job.task_count,
        max_parallel: job.max_parallel,
        adapter: job.adapter.clone(),
        layers,
        tasks,
        planner_log_tail: read_log_tail(config, &job.job_id, log_max),
        created_at: job.created_at.to_rfc3339(),
        updated_at: job.updated_at.to_rfc3339(),
    })
}

pub fn load_proposed(config: &Config, job_id: &str) -> Result<PlanIR> {
    let path = job_dir(config, job_id).join("plan.proposed.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("missing plan.proposed.json for {job_id}"))?;
    let ir: PlanIR = serde_json::from_str(&text)?;
    ir.validate()?;
    Ok(ir)
}

fn write_proposed(config: &Config, job_id: &str, ir: &PlanIR) -> Result<()> {
    let path = job_dir(config, job_id).join("plan.proposed.json");
    std::fs::write(&path, serde_json::to_string_pretty(ir)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Create job and run planner synchronously (parse/fake are fast; ai heuristic too).
pub fn start_plan_job(config: &Config, req: StartPlanJobRequest) -> Result<PlanJobView> {
    if !req.project.is_dir() {
        bail!("项目路径不是目录: {}", req.project.display());
    }
    let plan_mode = req
        .plan_mode
        .as_deref()
        .unwrap_or("parse")
        .trim()
        .to_ascii_lowercase();
    if !matches!(plan_mode.as_str(), "parse" | "fake" | "ai") {
        bail!("未知 plan_mode: {plan_mode}（支持 parse|fake|ai）");
    }
    let provider = req
        .provider
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.default.default_provider.clone());
    let exec_mode = req
        .mode
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.default.default_mode.clone());

    let job_id = new_job_id();
    let project = req
        .project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", req.project.display()))?;
    let now = Utc::now();
    let mut job = PlanJob {
        job_id: job_id.clone(),
        status: PlanJobStatus::Planning,
        project: project.clone(),
        plan_path: req.plan.clone(),
        plan_mode: plan_mode.clone(),
        provider: provider.clone(),
        exec_mode: exec_mode.clone(),
        error: None,
        run_id: None,
        created_at: now,
        updated_at: now,
        plan_name: None,
        task_count: None,
        max_parallel: None,
        adapter: None,
    };
    std::fs::create_dir_all(job_dir(config, &job_id))?;
    job.save(config)?;
    append_log(
        config,
        &job_id,
        &format!(
            "plan job started mode={plan_mode} project={} plan={}",
            project.display(),
            req.plan.display()
        ),
    );

    // `ai` may call Claude and take minutes — run in background so UI can poll.
    let async_ai = plan_mode == "ai";
    if async_ai {
        let cfg = config.clone();
        let jid = job_id.clone();
        std::thread::spawn(move || {
            let mut job = match PlanJob::load(&cfg, &jid) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(error = %e, "plan job load failed in worker");
                    return;
                }
            };
            finish_plan_job(&cfg, &mut job);
        });
        // Return immediately while planning
        let job = PlanJob::load(config, &job_id)?;
        return job_view(config, &job, 48_000);
    }

    finish_plan_job(config, &mut job);
    let job = PlanJob::load(config, &job_id)?;
    job_view(config, &job, 48_000)
}

fn finish_plan_job(config: &Config, job: &mut PlanJob) {
    let job_id = job.job_id.clone();
    match run_planner(config, job) {
        Ok(ir) => {
            if let Err(e) = write_proposed(config, &job_id, &ir) {
                job.status = PlanJobStatus::PlanFailed;
                job.error = Some(e.to_string());
                job.updated_at = Utc::now();
                let _ = job.save(config);
                append_log(config, &job_id, &format!("write proposed failed: {e:#}"));
                return;
            }
            job.status = PlanJobStatus::Planned;
            job.plan_name = Some(ir.name.clone());
            job.task_count = Some(ir.tasks.len());
            job.max_parallel = Some(ir.max_parallel);
            job.adapter = Some(ir.adapter.clone());
            job.error = None;
            job.updated_at = Utc::now();
            let _ = job.save(config);
            append_log(
                config,
                &job_id,
                &format!(
                    "planned ok name={} tasks={} max_parallel={} layers={}",
                    ir.name,
                    ir.tasks.len(),
                    ir.max_parallel,
                    topo_layers(&ir).len()
                ),
            );
            for (i, layer) in topo_layers(&ir).iter().enumerate() {
                append_log(
                    config,
                    &job_id,
                    &format!("  wave {}: {}", i + 1, layer.join(", ")),
                );
            }
        }
        Err(e) => {
            job.status = PlanJobStatus::PlanFailed;
            job.error = Some(e.to_string());
            job.updated_at = Utc::now();
            let _ = job.save(config);
            append_log(config, &job_id, &format!("plan failed: {e:#}"));
        }
    }
}

pub fn get_plan_job(config: &Config, job_id: &str) -> Result<PlanJobView> {
    let job = PlanJob::load(config, job_id)?;
    job_view(config, &job, 96_000)
}

fn run_planner(config: &Config, job: &mut PlanJob) -> Result<PlanIR> {
    match job.plan_mode.as_str() {
        "parse" => {
            append_log(config, &job.job_id, "using adapter parse (load_plan)");
            let mut ir = load_plan(&job.project, &job.plan_path, None, config)?;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        "fake" => {
            append_log(config, &job.job_id, "using fake multi-task demo DAG");
            Ok(build_fake_plan(config, job)?)
        }
        "ai" => {
            // Prefer structured parse when the document already has a graph.
            match load_plan(&job.project, &job.plan_path, None, config) {
                Ok(mut ir) if ir.adapter != "raw-single" && ir.tasks.len() > 1 => {
                    append_log(
                        config,
                        &job.job_id,
                        &format!(
                            "document already structured (adapter={}), skipping LLM",
                            ir.adapter
                        ),
                    );
                    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
                    return Ok(ir);
                }
                Ok(ir) if ir.adapter != "raw-single" => {
                    append_log(
                        config,
                        &job.job_id,
                        &format!("structured adapter={}, keeping as-is", ir.adapter),
                    );
                    let mut ir = ir;
                    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
                    return Ok(ir);
                }
                _ => {}
            }

            // 1) Try real Claude planner (skip when demo/fake or forced heuristic)
            let force_heuristic = std::env::var("CCO_PLANNER_HEURISTIC")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
                || job.provider == "fake";
            if !force_heuristic {
                match build_llm_plan(config, job) {
                    Ok(mut ir) => {
                        apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
                        return Ok(ir);
                    }
                    Err(e) => {
                        append_log(
                            config,
                            &job.job_id,
                            &format!("LLM planner failed ({e:#}); falling back to heuristic"),
                        );
                    }
                }
            } else {
                append_log(
                    config,
                    &job.job_id,
                    "skipping LLM planner (fake provider or CCO_PLANNER_HEURISTIC)",
                );
            }

            // 2) Heuristic fallback
            append_log(
                config,
                &job.job_id,
                "using ai heuristic splitter (heading/paragraph)",
            );
            let mut ir = build_heuristic_ai_plan(config, job)?;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        other => bail!("unsupported plan_mode {other}"),
    }
}

/// Call Claude CLI (print) to produce a cco-plan/v1 JSON task graph.
fn build_llm_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    use crate::runtime::provider::{
        claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
    };

    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let source_text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    // Cap source size for prompt budget
    let source_text = if source_text.len() > 40_000 {
        format!(
            "{}…\n\n[truncated, {} bytes total]",
            &source_text[..40_000],
            source_text.len()
        )
    } else {
        source_text
    };

    let bin = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| {
            std::env::var("CCO_CLAUDE_BIN").unwrap_or_else(|_| "claude".into())
        });
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = job_dir(config, &job.job_id).join("llm_work");
    let task_dir = work.join("tasks").join("__planner__");
    std::fs::create_dir_all(&task_dir)?;

    let prompt = planner_system_prompt(&job.project, &source_text, config.default.max_parallel);
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;
    append_log(config, &job.job_id, "starting Claude LLM planner (print)…");

    let planner_task = TaskIR {
        id: "__planner__".into(),
        title: "plan split".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        acceptance: None,
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            "max_turns": 8,
            "max_budget_usd": 2.0,
            "permission_mode": "dontAsk",
            "allowed_tools": [],
        }),
    };

    let ctx = StartCtx {
        run_id: job.job_id.clone(),
        project_root: job.project.clone(),
        work_dir: job.project.clone(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for llm planner")?;

    let raw_out = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&planner_task)?;
        let handle = provider.start(&planner_task, &ctx).await?;
        // Poll until done (max ~10 min already in task timeout)
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done | WorkerStatus::Failed | WorkerStatus::Stopped | WorkerStatus::Timeout => {
                    break;
                }
            }
        }
        let result = provider.collect(&handle).await?;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result.error.unwrap_or_else(|| "planner worker failed".into());
            bail!("planner worker not done: {err}\n{stdout}");
        }
        Ok::<String, anyhow::Error>(stdout)
    })?;

    append_log(
        config,
        &job.job_id,
        &format!("LLM raw output {} bytes", raw_out.len()),
    );
    // Keep a copy for debug
    let _ = std::fs::write(job_dir(config, &job.job_id).join("llm_raw.txt"), &raw_out);

    let mut ir = parse_llm_plan_output(&raw_out, &abs, config)?;
    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
    ir.adapter = "planner-ai-llm".into();
    ir.source_path = abs;
    ir.validate()?;
    append_log(
        config,
        &job.job_id,
        &format!("LLM plan ok: {} tasks", ir.tasks.len()),
    );
    Ok(ir)
}

fn planner_system_prompt(project: &Path, source: &str, max_parallel: usize) -> String {
    format!(
        r#"你是 cco 编排器的「规划器」。根据用户计划文档，拆成可并行的多任务 DAG。

项目路径: {project}
max_parallel 建议上限: {max_parallel}

## 输出要求（必须遵守）
1. 只输出 **一个** JSON 对象，不要 Markdown 解释，不要用 ``` 包裹以外的多余文字（若必须用代码块，仅一个 json 代码块）。
2. JSON schema:
{{
  "schema": "cco-plan/v1",
  "name": "短名称",
  "max_parallel": 2,
  "on_failure": "pause",
  "tasks": [
    {{
      "id": "t1",
      "title": "中文短标题",
      "depends_on": [],
      "prompt": "给执行 worker 的完整中文说明，自包含，结尾要求输出 CCO_DONE ok"
    }}
  ]
}}
3. 任务数 2–12；能并行的不要串成一条长链。
4. id 仅用 [a-z0-9_-]，稳定且唯一。
5. depends_on 只能引用已有 id，无环。
6. 每个 prompt 必须自包含（worker 看不到其它任务对话）。

## 用户计划文档
{source}
"#,
        project = project.display(),
        max_parallel = max_parallel.max(1),
        source = source,
    )
}

#[derive(Debug, Deserialize)]
struct LlmPlanDoc {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    max_parallel: Option<usize>,
    #[serde(default)]
    on_failure: Option<String>,
    tasks: Vec<LlmTask>,
}

#[derive(Debug, Deserialize)]
struct LlmTask {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    prompt: String,
    #[serde(default)]
    group: Option<String>,
}

fn parse_llm_plan_output(raw: &str, source_path: &Path, config: &Config) -> Result<PlanIR> {
    let json_str = extract_json_object(raw).context("LLM 输出中未找到 JSON 对象")?;
    let doc: LlmPlanDoc = serde_json::from_str(&json_str)
        .with_context(|| format!("parse planner JSON: {}", &json_str.chars().take(200).collect::<String>()))?;
    if doc.tasks.is_empty() {
        bail!("LLM plan has no tasks");
    }
    if doc.tasks.len() > 20 {
        bail!("LLM plan has too many tasks ({})", doc.tasks.len());
    }
    let name = doc
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("planned")
                .to_string()
        });
    let provider = config.default.default_provider.clone();
    let opts = default_provider_opts(config, &provider);
    let on_failure = match doc.on_failure.as_deref().unwrap_or("pause") {
        "continue" => OnFailure::Continue,
        "retry" => OnFailure::Retry,
        _ => OnFailure::Pause,
    };
    let tasks: Vec<TaskIR> = doc
        .tasks
        .into_iter()
        .map(|t| {
            let title = t
                .title
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| t.id.clone());
            let mut prompt = t.prompt;
            if !prompt.contains("CCO_DONE") {
                prompt.push_str("\n\n全部完成后在最后一行输出：CCO_DONE ok\n");
            }
            TaskIR {
                id: t.id,
                title,
                depends_on: t.depends_on,
                group: t.group,
                provider: provider.clone(),
                mode: config.default.default_mode.clone(),
                prompt,
                acceptance: None,
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: opts.clone(),
            }
        })
        .collect();

    Ok(PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "planner-ai-llm".into(),
        source_path: source_path.to_path_buf(),
        max_parallel: doc
            .max_parallel
            .unwrap_or(config.default.max_parallel)
            .clamp(1, 32),
        on_failure,
        retry_max: 0,
        default_provider: provider,
        default_mode: config.default.default_mode.clone(),
        worktree: false,
        tasks,
    })
}

fn extract_json_object(raw: &str) -> Option<String> {
    // Prefer fenced ```json ... ```
    if let Some(idx) = raw.find("```") {
        let after = &raw[idx + 3..];
        let after = after
            .strip_prefix("json")
            .or_else(|| after.strip_prefix("JSON"))
            .unwrap_or(after);
        let after = after.trim_start_matches(|c| c == '\n' || c == '\r');
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if block.starts_with('{') {
                return Some(block.to_string());
            }
        }
    }
    // Fallback: first { to last }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end > start {
        Some(raw[start..=end].to_string())
    } else {
        None
    }
}

fn apply_worker_defaults(ir: &mut PlanIR, provider: &str, exec_mode: &str) {
    ir.default_provider = provider.to_string();
    ir.default_mode = exec_mode.to_string();
    for t in &mut ir.tasks {
        t.provider = provider.to_string();
        t.mode = exec_mode.to_string();
    }
}

fn build_fake_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("demo")
        .to_string();
    let opts = default_provider_opts(config, &job.provider);
    let src_hint = job.plan_path.display().to_string();

    let mk = |id: &str, title: &str, deps: Vec<&str>, group: &str, body: &str| TaskIR {
        id: id.into(),
        title: title.into(),
        depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        group: Some(group.into()),
        provider: job.provider.clone(),
        mode: job.exec_mode.clone(),
        prompt: format!(
            "【模拟任务 {id}】{title}\n来源计划: {src_hint}\n\n{body}\n\n完成后输出一行: CCO_DONE ok\n"
        ),
        acceptance: None,
        timeout_secs: Some(120),
        worktree: Some(false),
        provider_opts: opts.clone(),
    };

    let ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name: format!("{name}-fake"),
        adapter: "planner-fake".into(),
        source_path: abs,
        max_parallel: 2.min(config.default.max_parallel.max(1)),
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: job.provider.clone(),
        default_mode: job.exec_mode.clone(),
        worktree: false,
        tasks: vec![
            mk(
                "t1",
                "调研与范围",
                vec![],
                "G1",
                "阅读计划意图，列出 3 条范围说明（模拟，无需改仓库）。",
            ),
            mk(
                "t2",
                "脚手架",
                vec![],
                "G1",
                "描述将创建的文件清单（模拟，无需真实写入）。",
            ),
            mk(
                "t3",
                "实现与集成",
                vec!["t1", "t2"],
                "G2",
                "在 t1/t2 完成后做集成说明（模拟）。",
            ),
            mk(
                "t4",
                "验收摘要",
                vec!["t3"],
                "G3",
                "输出验收检查表（模拟）。",
            ),
        ],
    };
    ir.validate()?;
    // tiny delay so UI can show planning state if polled mid-flight (sync path still fine)
    let _ = Duration::from_millis(1);
    Ok(ir)
}

/// Split markdown-ish text into tasks by `##` / `###` headings; sequential deps.
fn build_heuristic_ai_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("planned")
        .to_string();
    let opts = default_provider_opts(config, &job.provider);

    let sections = split_sections(&text);
    append_log(
        config,
        &job.job_id,
        &format!("heuristic found {} section(s)", sections.len()),
    );

    let sections = if sections.len() <= 1 {
        // Fall back to chunking long prose into up to 3 sequential tasks.
        chunk_prose(&text, 3)
    } else {
        sections
    };

    if sections.is_empty() {
        bail!("计划文档为空，无法拆分");
    }

    let mut tasks = Vec::new();
    for (i, (title, body)) in sections.iter().enumerate() {
        let id = format!("t{}", i + 1);
        let depends_on = if i == 0 {
            vec![]
        } else {
            vec![format!("t{i}")]
        };
        let prompt = format!(
            "你是执行任务 `{id}`（{title}）的 worker。\n\
             项目根目录即当前工作目录。\n\
             依据下列说明完成工作；不要做范围外改动。\n\n\
             ## 任务说明\n{body}\n\n\
             全部完成后在最后一行输出：CCO_DONE ok\n"
        );
        tasks.push(TaskIR {
            id,
            title: title.clone(),
            depends_on,
            group: Some(format!("G{}", i + 1)),
            provider: job.provider.clone(),
            mode: job.exec_mode.clone(),
            prompt,
            acceptance: None,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: opts.clone(),
        });
    }

    // Independent first two when many sections? Keep sequential for safety in heuristic.
    let max_parallel = 1.max(config.default.max_parallel.min(2));
    let ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "planner-ai-heuristic".into(),
        source_path: abs,
        max_parallel,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: job.provider.clone(),
        default_mode: job.exec_mode.clone(),
        worktree: false,
        tasks,
    };
    ir.validate()?;
    Ok(ir)
}

fn split_sections(text: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur_body = String::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            if let Some(t) = cur_title.take() {
                sections.push((t, cur_body.trim().to_string()));
                cur_body.clear();
            } else if !cur_body.trim().is_empty() {
                sections.push(("前言".into(), cur_body.trim().to_string()));
                cur_body.clear();
            }
            cur_title = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            // skip common meta headings
            let t = rest.trim();
            if matches!(
                t.to_ascii_lowercase().as_str(),
                "graph" | "tasks" | "目录" | "toc"
            ) {
                continue;
            }
            if let Some(prev) = cur_title.take() {
                sections.push((prev, cur_body.trim().to_string()));
                cur_body.clear();
            } else if !cur_body.trim().is_empty() {
                sections.push(("前言".into(), cur_body.trim().to_string()));
                cur_body.clear();
            }
            cur_title = Some(t.to_string());
        } else {
            cur_body.push_str(line);
            cur_body.push('\n');
        }
    }
    if let Some(t) = cur_title {
        sections.push((t, cur_body.trim().to_string()));
    } else if !cur_body.trim().is_empty() {
        sections.push(("全文".into(), cur_body.trim().to_string()));
    }
    sections.retain(|(_, b)| !b.is_empty());
    sections
}

fn chunk_prose(text: &str, max_parts: usize) -> Vec<(String, String)> {
    let paras: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with("---"))
        .collect();
    if paras.is_empty() {
        let t = text.trim();
        if t.is_empty() {
            return vec![];
        }
        return vec![("全文任务".into(), t.to_string())];
    }
    let n = max_parts.min(paras.len()).max(1);
    let chunk = (paras.len() + n - 1) / n;
    let mut out = Vec::new();
    for (i, part) in paras.chunks(chunk).enumerate() {
        if i >= max_parts {
            break;
        }
        let body = part.join("\n\n");
        let title = first_line_title(part[0], i);
        out.push((title, body));
    }
    out
}

fn first_line_title(para: &str, idx: usize) -> String {
    let line = para.lines().next().unwrap_or("任务").trim();
    let cleaned = line
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(40)
        .collect::<String>();
    if cleaned.is_empty() {
        format!("任务 {}", idx + 1)
    } else {
        cleaned
    }
}

/// Mark job confirmed after exec run was spawned (called from services).
pub fn mark_confirmed(config: &Config, job_id: &str, run_id: &str, ir: &PlanIR) -> Result<()> {
    let mut job = PlanJob::load(config, job_id)?;
    if !matches!(job.status, PlanJobStatus::Planned) {
        bail!(
            "计划任务状态为 {}，只有「待确认/planned」才能确认",
            job.status.as_str()
        );
    }
    job.status = PlanJobStatus::Confirmed;
    job.run_id = Some(run_id.to_string());
    job.updated_at = Utc::now();
    job.save(config)?;
    append_log(config, job_id, &format!("confirmed run_id={run_id}"));
    let _ = std::fs::write(
        job_dir(config, job_id).join("plan.resolved.json"),
        serde_json::to_string_pretty(ir)?,
    );
    Ok(())
}

/// Load proposed plan and apply job's provider/mode defaults.
pub fn load_proposed_for_exec(config: &Config, job_id: &str) -> Result<(PlanJob, PlanIR)> {
    let job = PlanJob::load(config, job_id)?;
    if !matches!(job.status, PlanJobStatus::Planned) {
        bail!(
            "计划任务状态为 {}，只有「待确认/planned」才能开始运行",
            job.status.as_str()
        );
    }
    let mut ir = load_proposed(config, job_id)?;
    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
    ir.validate()?;
    append_log(
        config,
        job_id,
        &format!("confirm_start → spawning run with {} tasks", ir.tasks.len()),
    );
    Ok((job, ir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fake_plan_validates() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# hello\n\ndo something cool\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        assert!(view.task_count.unwrap() >= 3);
        assert!(!view.layers.is_empty());
        assert_eq!(view.layers[0].len(), 2); // t1,t2 parallel
    }

    #[test]
    fn heuristic_splits_headings() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("spec.md");
        std::fs::write(
            &plan,
            "## 准备\n\n写 README\n\n## 功能\n\n实现 foo\n\n## 测试\n\n补测试\n",
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project,
                plan: PathBuf::from("spec.md"),
                plan_mode: Some("ai".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
            },
        )
        .unwrap();
        // ai mode is async (LLM attempt); poll until planned/failed
        let mut view = view;
        for _ in 0..100 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            view = get_plan_job(&cfg, &view.job_id).unwrap();
        }
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        assert_eq!(view.task_count, Some(3));
        assert_eq!(view.layers.len(), 3);
    }

    #[test]
    fn confirm_starts_run_dir() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(cfg.runs_dir()).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        let run_id = crate::services::confirm_start(cfg.clone(), &view.job_id).unwrap();
        assert!(!run_id.is_empty());
        let job = PlanJob::load(&cfg, &view.job_id).unwrap();
        assert_eq!(job.status, PlanJobStatus::Confirmed);
        assert_eq!(job.run_id.as_deref(), Some(run_id.as_str()));
        // run state file exists
        assert!(cfg.runs_dir().join(&run_id).join("run.json").exists());
        // give scheduler a moment
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
