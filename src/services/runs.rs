//! Run lifecycle: list/load/start/stop/resume + plan-job confirm.
//!
//! [INPUT]: Config · StartRunRequest · PlanIR · plan job id
//! [OUTPUT]: RunSummary · start_run_* · confirm_start · stop_run · resume_run_async ·
//!           start_rework_from_run · accept_run_residual（P-loop）
//! [POS]: services 子模块；Mode B confirm_start 唯一业务入口；rework 另起 run
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::doctor::{self, DoctorReport};
use crate::plan::{self, load_plan, PlanIR};
use crate::report;
use crate::runtime::handoff::{
    self, accept_residual_on_handoff, build_rework_plan, count_rework_rounds,
    load_parsed_inspect_issues, REWORK_MAX_ROUNDS,
};
use crate::runtime::provider::{ProviderRegistry, TaskStatus};
use crate::runtime::Scheduler;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

use super::util::kill_pid;

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub status: String,
    pub project_root: String,
    pub plan_path: String,
    pub started_at: String,
    pub task_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRunRequest {
    pub project: PathBuf,
    pub plan: PathBuf,
    pub provider: String,
    pub mode: String,
}

/// Lightweight plan summary for the UI (no worker spin-up needed).
#[derive(Debug, Clone, Serialize)]
pub struct PlanPreview {
    pub name: String,
    pub schema: String,
    pub adapter: String,
    pub task_count: usize,
    pub task_titles: Vec<String>,
    pub max_parallel: usize,
    pub on_failure: String,
    pub worktree: bool,
}

pub fn list_runs(config: &Config) -> Result<Vec<RunSummary>> {
    let root = config.runs_dir();
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("run.json").exists())
        .collect();
    dirs.sort();
    dirs.reverse();
    for d in dirs.into_iter().take(80) {
        if let Ok(rs) = RunState::load(&d) {
            out.push(RunSummary {
                run_id: rs.run_id,
                status: format!("{:?}", rs.status).to_ascii_lowercase(),
                project_root: rs.project_root.display().to_string(),
                plan_path: rs.plan_path.display().to_string(),
                started_at: rs.started_at.to_rfc3339(),
                task_count: rs.tasks.len(),
            });
        }
    }
    Ok(out)
}

pub fn load_run(config: &Config, run_id: &str) -> Result<RunState> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    RunState::load(&dir)
}

pub fn list_plans(project: &Path) -> Result<Vec<String>> {
    let plans = plan::list_plans(project)?;
    Ok(plans
        .iter()
        .map(|p| {
            p.strip_prefix(project)
                .map(|r| r.display().to_string())
                .unwrap_or_else(|_| p.display().to_string())
        })
        .collect())
}

/// Lightweight plan preview without spinning up providers.
pub fn preview_plan(project: &Path, plan_rel: &Path, config: &Config) -> Result<PlanPreview> {
    let ir = load_plan(project, plan_rel, None, config)?;
    Ok(PlanPreview {
        name: ir.name,
        schema: ir.schema,
        adapter: ir.adapter,
        task_count: ir.tasks.len(),
        task_titles: ir.tasks.iter().map(|t| t.title.clone()).collect(),
        max_parallel: ir.max_parallel,
        on_failure: format!("{:?}", ir.on_failure).to_ascii_lowercase(),
        worktree: ir.worktree,
    })
}

pub async fn run_doctor(config: &Config, project: Option<&Path>) -> Result<DoctorReport> {
    doctor::run_doctor(config, project).await
}

/// Start a run on a background tokio task; returns run_id immediately.
/// Loads plan from disk (legacy path). Prefer plan-job `confirm_start` for mode B.
pub fn start_run_async(config: Config, req: StartRunRequest) -> Result<String> {
    if !req.project.is_dir() {
        bail!("项目路径不是目录: {}", req.project.display());
    }
    let mut ir = load_plan(&req.project, &req.plan, None, &config)?;
    for t in &mut ir.tasks {
        t.provider = req.provider.clone();
        t.mode = req.mode.clone();
    }
    ir.default_provider = req.provider.clone();
    ir.default_mode = req.mode.clone();
    let project = req.project.canonicalize().context("canonicalize project")?;
    start_run_from_plan(config, project, &ir)
}

/// Start scheduler from an already-built PlanIR (used by plan-job confirm).
pub fn start_run_from_plan(config: Config, project: PathBuf, ir: &PlanIR) -> Result<String> {
    if !project.is_dir() {
        bail!("项目路径不是目录: {}", project.display());
    }
    ir.validate()?;

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let project = project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", project.display()))?;
    let run_state = RunState::new(run_id.clone(), project, ir, run_dir.clone());
    run_state.save()?;

    // Persist resolved plan for resume (scheduler also writes this; write early for UI).
    let resolved = run_dir.join("plan.resolved.json");
    std::fs::write(&resolved, serde_json::to_string_pretty(ir)?)?;

    let registry = ProviderRegistry::from_config(&config)?;
    let max_parallel = ir.max_parallel;
    let tm = TerminalManager::for_run(
        &run_dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    );
    let provider_caps: HashMap<String, usize> = config
        .providers
        .iter()
        .filter_map(|(n, pc)| pc.max_parallel.map(|m| (n.clone(), m)))
        .collect();
    let budget = config.default.run_max_budget_usd;
    let runs_dir = config.runs_dir();
    let rid = run_id.clone();
    let ir = ir.clone();
    let poll_secs = config.default.poll_interval_secs.clamp(1, 30);
    let retry_max = config.default.retry_max;
    let stall_secs = config.default.stall_secs;

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async move {
            for name in ir
                .tasks
                .iter()
                .map(|t| t.provider.as_str())
                .collect::<std::collections::HashSet<_>>()
            {
                if let Ok(p) = registry.get(name) {
                    if let Err(e) = p.preflight().await {
                        tracing::error!(provider = name, error = %e, "preflight failed");
                        return;
                    }
                }
            }
            let sched = Scheduler {
                max_parallel,
                plan: ir,
                state: run_state,
                registry,
                poll_interval: Duration::from_secs(poll_secs),
                yes: true,
                only: None,
                from_task: None,
                dry_run: false,
                mirror_state: None,
                auto_open_terminal: false,
                terminal_kind: SessionKind::Embedded,
                terminal_manager: Some(tm),
                run_max_budget_usd: budget,
                provider_max_parallel: provider_caps,
                retry_max,
                stall_secs,
            };
            match sched.run().await {
                Ok(status) => {
                    tracing::info!(%rid, ?status, "desktop run finished");
                    if let Ok(rs) = RunState::load(&runs_dir.join(&rid)) {
                        let _ = report::write_reports(&rs);
                    }
                }
                Err(e) => tracing::error!(%rid, error = %e, "desktop run failed"),
            }
        });
    });

    Ok(run_id)
}

// ── Plan job (mode B) ──────────────────────────────────────────────

pub use crate::plan::planner::{
    get_plan_job, latest_plan_job_for_project, start_plan_job, update_proposed_task, PlanJobView,
    StartPlanJobRequest,
};

/// Freeze proposed plan and start scheduler; returns run_id.
pub fn confirm_start(config: Config, job_id: &str) -> Result<String> {
    let (job, ir) = crate::plan::planner::load_proposed_for_exec(&config, job_id)?;
    let run_id = start_run_from_plan(config.clone(), job.project.clone(), &ir)?;
    crate::plan::planner::mark_confirmed(&config, job_id, &run_id, &ir)?;
    Ok(run_id)
}

pub fn stop_run(config: &Config, run_id: &str) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;
    for (tid, ts) in rs.tasks.iter_mut() {
        if matches!(
            ts.status,
            TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
        ) {
            if let Some(pid) = ts.pid {
                kill_pid(pid);
            }
            let _ = std::fs::write(dir.join("tasks").join(tid).join(".done"), "130");
            ts.status = TaskStatus::Stopped;
            ts.finished_at = Some(chrono::Utc::now());
        }
    }
    rs.status = RunStatus::Aborted;
    rs.finished_at = Some(chrono::Utc::now());
    rs.save()?;
    let _ = rs.event(
        "run_end",
        serde_json::json!({"status": "aborted", "via": "desktop"}),
    );
    Ok(())
}

pub fn resume_run_async(config: Config, run_id: &str) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;
    if matches!(rs.status, RunStatus::Running) {
        bail!("run 仍在运行中，请先停止");
    }
    let plan_path = dir.join("plan.resolved.json");
    if !plan_path.exists() {
        bail!("缺少 plan.resolved.json");
    }
    let ir: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?;
    let _n = rs.prepare_for_resume();
    for (id, ts) in &rs.tasks {
        if matches!(ts.status, TaskStatus::Pending) {
            let _ = std::fs::remove_file(dir.join("tasks").join(id).join(".done"));
        }
    }
    rs.save()?;

    let registry = ProviderRegistry::from_config(&config)?;
    let max_parallel = ir.max_parallel;
    let tm = TerminalManager::for_run(
        &dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    );
    let provider_caps: HashMap<String, usize> = config
        .providers
        .iter()
        .filter_map(|(n, pc)| pc.max_parallel.map(|m| (n.clone(), m)))
        .collect();
    let budget = config.default.run_max_budget_usd;
    let runs_dir = config.runs_dir();
    let rid = rs.run_id.clone();
    let poll_secs = config.default.poll_interval_secs.clamp(1, 30);
    let retry_max = config.default.retry_max;
    let stall_secs = config.default.stall_secs;

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio");
        rt.block_on(async move {
            let sched = Scheduler {
                max_parallel,
                plan: ir,
                state: rs,
                registry,
                poll_interval: Duration::from_secs(poll_secs),
                yes: true,
                only: None,
                from_task: None,
                dry_run: false,
                mirror_state: None,
                auto_open_terminal: false,
                terminal_kind: SessionKind::Embedded,
                terminal_manager: Some(tm),
                run_max_budget_usd: budget,
                provider_max_parallel: provider_caps,
                retry_max,
                stall_secs,
            };
            let _ = sched.run().await;
            if let Ok(st) = RunState::load(&runs_dir.join(&rid)) {
                let _ = report::write_reports(&st);
            }
        });
    });
    Ok(())
}

// ── P-loop: rework wave + accept residual ────────────────────────────

/// Response for `start_rework_from_run` (desktop / CLI).
#[derive(Debug, Clone, Serialize)]
pub struct ReworkStartResponse {
    pub run_id: String,
    pub source_run_id: String,
    pub round: u32,
    pub max_rounds: u32,
    pub issue_count: usize,
    pub message: String,
}

/// Generate a rework PlanIR from inspect ISSUES of `source_run_id` and start a new run.
///
/// Rounds capped at [`REWORK_MAX_ROUNDS`]. Does not auto-merge/PR.
pub fn start_rework_from_run(config: Config, source_run_id: &str) -> Result<ReworkStartResponse> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(source_run_id))?;
    let rs = RunState::load(&dir)?;
    if matches!(rs.status, RunStatus::Running | RunStatus::Validated | RunStatus::Init) {
        bail!("源 run 仍在进行中，请等待结束后再回补");
    }
    let plan_path = dir.join("plan.resolved.json");
    if !plan_path.exists() {
        bail!("缺少 plan.resolved.json");
    }
    let base: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?;
    let project = rs.project_root.clone();

    let inspect_task = base
        .tasks
        .iter()
        .rev()
        .find(|t| t.role == Some(crate::plan::TaskRole::Inspect))
        .cloned();
    let issues = if let Some(ref t) = inspect_task {
        load_parsed_inspect_issues(t, &project, &project)
    } else {
        let path = project.join(handoff::INSPECT_ISSUES_REL);
        if path.is_file() {
            handoff::parse_issues_text(&std::fs::read_to_string(&path)?)
        } else {
            vec![]
        }
    };
    if issues.is_empty() {
        // Still allow rework from VERDICT=FAIL with empty ISSUES body — synthetic issue.
        let verdict_fail = inspect_task
            .as_ref()
            .map(|t| {
                handoff::read_inspect_verdict(t, &project, &project)
                    == handoff::InspectVerdict::Fail
            })
            .unwrap_or(false);
        if !verdict_fail {
            bail!("无 ISSUES 可回补；请先有巡检 FAIL 或 blocking 项");
        }
    }

    let mut issues = issues;
    if issues.is_empty() {
        issues.push(handoff::ParsedIssue {
            id: "I-verdict".into(),
            severity: handoff::IssueSeverity::Blocking,
            plan_ref: "inspect".into(),
            path: handoff::INSPECT_VERDICT_REL.into(),
            symptom: "VERDICT=FAIL without structured ISSUES".into(),
            fix_wp: "Read VERDICT.md and fix root causes; write progress evidence".into(),
            raw: "severity=blocking VERDICT=FAIL (no ISSUES body)".into(),
        });
    }

    let prior = count_rework_rounds(&project, &dir);
    let round = prior + 1;
    if round > REWORK_MAX_ROUNDS {
        bail!(
            "回补轮次已达上限 {REWORK_MAX_ROUNDS}；请人工处理或「接受残留」"
        );
    }

    let rework_ir = build_rework_plan(&base, &issues, round, source_run_id)?;
    // Record wave on source handoff timeline (best-effort).
    if let Ok(mut h) = handoff::Handoff::load(&dir) {
        h.timeline.push(format!(
            "{} · rework_wave · round={round} · issues={} · next plan={}",
            chrono::Utc::now().to_rfc3339(),
            issues.len(),
            rework_ir.name
        ));
        let _ = h.save(&dir);
    }
    // Also write a marker under project rework dir for round counting.
    let rework_dir = project.join(".cco-out/rework");
    let _ = std::fs::create_dir_all(&rework_dir);
    let _ = std::fs::write(
        rework_dir.join(format!("ROUND-{round}.queued.md")),
        format!(
            "# Rework round {round}\n\nsource_run: {source_run_id}\nissues: {}\n",
            issues.len()
        ),
    );

    let new_run_id = start_run_from_plan(config, project, &rework_ir)?;
    Ok(ReworkStartResponse {
        run_id: new_run_id,
        source_run_id: source_run_id.into(),
        round,
        max_rounds: REWORK_MAX_ROUNDS,
        issue_count: issues.len(),
        message: format!(
            "已生成第 {round}/{REWORK_MAX_ROUNDS} 轮回补并启动（{} 条 ISSUE）",
            issues.len()
        ),
    })
}

/// User explicitly accepts residual / open risks (P-loop Q7). Writes handoff open_risks.
pub fn accept_run_residual(config: &Config, run_id: &str, note: Option<&str>) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let rs = RunState::load(&dir)?;
    let plan_path = dir.join("plan.resolved.json");
    let plan: PlanIR = if plan_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?
    } else {
        bail!("缺少 plan.resolved.json");
    };
    accept_residual_on_handoff(&plan, &rs, note.unwrap_or(""))?;
    Ok(())
}
