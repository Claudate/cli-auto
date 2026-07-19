//! Plan job types, paths, start/get/latest lifecycle.
//!
//! [INPUT]: StartPlanJobRequest · Config
//! [OUTPUT]: PlanJob · start_plan_job · get_plan_job · latest_plan_job_for_project
//! [POS]: planner 子模块；Mode B job IO
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::topo_layers;
use crate::plan::{load_plan, PlanIR};
use crate::runtime::log_events;

use super::heuristic::{build_fake_plan, build_heuristic_ai_plan};
use super::llm::build_llm_plan;
use super::view::{job_view, write_proposed, PlanJobView};

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
    /// Planner LLM spend (USD), separate from worker run cost (P1-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartPlanJobRequest {
    pub project: PathBuf,
    pub plan: PathBuf,
    /// parse | fake | ai  (default: parse)
    pub plan_mode: Option<String>,
    pub provider: Option<String>,
    pub mode: Option<String>,
    /// Scheduler concurrency chosen at split time (1–32). Defaults to config.
    pub max_parallel: Option<usize>,
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

pub(super) fn append_log(config: &Config, job_id: &str, line: &str) {
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

pub(super) fn read_log_tail(config: &Config, job_id: &str, max_bytes: usize) -> String {
    let path = job_dir(config, job_id).join("planner.log");
    // 行边界 tail，避免半截行（与 worker log_events 同构）
    log_events::read_text_tail(&path, max_bytes).0
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
    // Chosen at split time so the confirm UI and DAG reflect user intent.
    let max_parallel = req
        .max_parallel
        .unwrap_or(config.default.max_parallel)
        .clamp(1, 32);

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
        max_parallel: Some(max_parallel),
        adapter: None,
        planner_cost_usd: None,
    };
    std::fs::create_dir_all(job_dir(config, &job_id))?;
    job.save(config)?;
    append_log(
        config,
        &job_id,
        &format!(
            "plan job started mode={plan_mode} max_parallel={max_parallel} project={} plan={}",
            project.display(),
            req.plan.display()
        ),
    );

    // `ai` may call Claude CLI (print) and take minutes — background + UI poll.
    // 若本机解析不到 claude bin，则同步跑启发式，避免 UI 空等异步轮询。
    let async_ai = plan_mode == "ai" && planner_should_try_llm(config, &provider);
    if async_ai {
        append_log(
            config,
            &job_id,
            "async planner: will invoke Claude CLI (print / stream-json)",
        );
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

    if plan_mode == "ai" {
        append_log(
            config,
            &job_id,
            "claude CLI not resolvable in this process; running heuristic splitter synchronously",
        );
    }
    finish_plan_job(config, &mut job);
    let job = PlanJob::load(config, &job_id)?;
    job_view(config, &job, 48_000)
}

fn planner_should_try_llm(config: &Config, provider_name: &str) -> bool {
    if provider_name == "fake" {
        return false;
    }
    if std::env::var("CCO_PLANNER_HEURISTIC")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return false;
    }
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let p = std::path::Path::new(&bin);
    p.is_file() || which::which(&bin).is_ok()
}

pub(super) fn finish_plan_job(config: &Config, job: &mut PlanJob) {
    let job_id = job.job_id.clone();
    match run_planner(config, job) {
        Ok(mut ir) => {
            // Split-time concurrency wins over planner defaults / document values.
            if let Some(n) = job.max_parallel {
                ir.max_parallel = n.clamp(1, 32);
            }
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
            // Planner cost from LLM path (if any); parse/fake leave None.
            if job.planner_cost_usd.is_none() {
                job.planner_cost_usd = super::llm::read_planner_cost(config, &job_id);
            }
            job.error = None;
            job.updated_at = Utc::now();
            let _ = job.save(config);
            append_log(
                config,
                &job_id,
                &format!(
                    "planned ok name={} tasks={} max_parallel={} layers={} planner_cost={:?}",
                    ir.name,
                    ir.tasks.len(),
                    ir.max_parallel,
                    topo_layers(&ir).len(),
                    job.planner_cost_usd
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

/// 查找项目最近可恢复的规划会话（planning / planned / confirmed 且有任务图）。
/// 用于进项目时接上「上次拆分结果」，避免每次重拆。
pub fn latest_plan_job_for_project(
    config: &Config,
    project: &Path,
) -> Result<Option<PlanJobView>> {
    let root = plan_jobs_dir(config);
    if !root.is_dir() {
        return Ok(None);
    }
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());

    let mut best: Option<PlanJob> = None;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let job_path = entry.path().join("job.json");
        if !job_path.is_file() {
            continue;
        }
        let job: PlanJob = match std::fs::read_to_string(&job_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(j) => j,
            None => continue,
        };
        let jp = job
            .project
            .canonicalize()
            .unwrap_or_else(|_| job.project.clone());
        if jp != project {
            continue;
        }
        // 只恢复仍有价值的状态
        match job.status {
            PlanJobStatus::Planning | PlanJobStatus::Planned | PlanJobStatus::Confirmed => {}
            PlanJobStatus::PlanFailed | PlanJobStatus::Cancelled => continue,
        }
        // confirmed 必须仍有图文件
        if matches!(job.status, PlanJobStatus::Confirmed | PlanJobStatus::Planned) {
            let dir = entry.path();
            if !dir.join("plan.proposed.json").is_file()
                && !dir.join("plan.resolved.json").is_file()
            {
                continue;
            }
        }
        let replace = match &best {
            None => true,
            Some(b) => job.updated_at > b.updated_at,
        };
        if replace {
            best = Some(job);
        }
    }

    match best {
        Some(job) => Ok(Some(job_view(config, &job, 96_000)?)),
        None => Ok(None),
    }
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
            // Prefer structured parse when the document already has a real work graph.
            // Spec / contract MD often falsely matches serial-prompts (### Board, ### P0…)
            // — treat meta-heavy / product-spec graphs as prose and re-split.
            let source_text = crate::plan::resolve_plan_path(&job.project, &job.plan_path)
                .ok()
                .and_then(|p| std::fs::read_to_string(p).ok());
            let is_spec = source_text
                .as_deref()
                .map(super::heuristic::looks_like_spec_document)
                .unwrap_or(false);

            match load_plan(&job.project, &job.plan_path, None, config) {
                Ok(mut ir) if ir.adapter != "raw-single" && !ir.tasks.is_empty() => {
                    let meta_n = ir
                        .tasks
                        .iter()
                        .filter(|t| {
                            crate::plan::title_is_meta_heading(&t.id)
                                || crate::plan::title_is_meta_heading(&t.title)
                        })
                        .count();
                    let meta_heavy = meta_n * 2 >= ir.tasks.len().max(1);
                    // serial-prompts without fenced worker prompts is almost always a false graph
                    let unfenced = ir.tasks.iter().filter(|t| !t.prompt.contains("```")).count();
                    let looks_like_false_graph = ir.adapter == "serial-prompts/v0"
                        && (is_spec || unfenced * 2 >= ir.tasks.len().max(1) || meta_heavy);

                    if is_spec || meta_heavy || looks_like_false_graph {
                        append_log(
                            config,
                            &job.job_id,
                            &format!(
                                "adapter={} tasks={} meta={} spec={} unfenced={} → re-planning as work orders",
                                ir.adapter,
                                ir.tasks.len(),
                                meta_n,
                                is_spec,
                                unfenced
                            ),
                        );
                    } else {
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

pub(super) fn apply_worker_defaults(ir: &mut PlanIR, provider: &str, exec_mode: &str) {
    ir.default_provider = provider.to_string();
    ir.default_mode = exec_mode.to_string();
    for t in &mut ir.tasks {
        t.provider = provider.to_string();
        t.mode = exec_mode.to_string();
    }
}
