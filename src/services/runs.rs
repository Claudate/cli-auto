//! Run lifecycle IO adapter (migration facade · A1-7).
//!
//! Presentation should call [`crate::app::run`] / [`crate::app::split`].
//! This module holds disk/scheduler IO; `confirm_start` is a one-line
//! facade over [`crate::app::split::confirm`].
//!
//! [INPUT]: Config · StartRunRequest · PlanIR · plan job id
//! [OUTPUT]: RunSummary · PlanMeta · list_plans/list_plan_meta · start_run_* · confirm_start ·
//!           stop_run · resume_run_async · retry_task_async · start_rework_from_run · accept_run_residual（P-loop）
//! [POS]: services 子模块；Mode B 开跑真源 = app::split::confirm；rework 另起 run
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md · **勿在此新增业务策略**

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

use super::util::{kill_pid, paths_match};

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

/// Plan list row with run-history meta (H2 / §4.5).
///
/// `ever_completed` is true iff at least one run bound to this plan_path
/// finished with [`RunStatus::Completed`]. Never inferred from file mtime.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlanMeta {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_finished_at: Option<String>,
    pub ever_completed: bool,
}

/// Per-plan aggregate while scanning runs (newest-first input).
#[derive(Debug, Default)]
struct PlanRunAgg {
    last_run_id: Option<String>,
    last_run_status: Option<String>,
    last_run_finished_at: Option<String>,
    ever_completed: bool,
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

/// List plans with run-history meta for chooser / plan-rail (H2).
///
/// Keeps path strings compatible with [`list_plans`]. Aggregates from
/// `~/.cco/runs/*/run.json` (`plan_path` + `status` + `finished_at`); does
/// **not** use file mtime to guess execution.
pub fn list_plan_meta(config: &Config, project: &Path) -> Result<Vec<PlanMeta>> {
    let paths = list_plans(project)?;
    let by_key = aggregate_plan_runs(config, project)?;

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let key = normalize_plan_key(project, Path::new(&path));
        let agg = by_key.get(&key);
        let title = plan_title_hint(project, &path);
        out.push(PlanMeta {
            path,
            title,
            last_run_id: agg.and_then(|a| a.last_run_id.clone()),
            last_run_status: agg.and_then(|a| a.last_run_status.clone()),
            last_run_finished_at: agg.and_then(|a| a.last_run_finished_at.clone()),
            ever_completed: agg.map(|a| a.ever_completed).unwrap_or(false),
        });
    }
    Ok(out)
}

/// Scan run.json files for a project and fold into per-plan aggregates.
///
/// Input order is newest-first (same as [`list_runs`]); first sighting of a
/// plan key fills `last_run_*`. Any `Completed` status sets `ever_completed`.
fn aggregate_plan_runs(config: &Config, project: &Path) -> Result<HashMap<String, PlanRunAgg>> {
    let root = config.runs_dir();
    let mut by_key: HashMap<String, PlanRunAgg> = HashMap::new();
    if !root.is_dir() {
        return Ok(by_key);
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("run.json").exists())
        .collect();
    dirs.sort();
    dirs.reverse();

    // Cap scan like list_runs; enough for recent history without walking forever.
    for d in dirs.into_iter().take(200) {
        let rs = match RunState::load(&d) {
            Ok(rs) => rs,
            Err(_) => continue,
        };
        if !paths_match(&rs.project_root, project) {
            continue;
        }
        let key = normalize_plan_key(project, &rs.plan_path);
        if key.is_empty() {
            continue;
        }
        let status = format!("{:?}", rs.status).to_ascii_lowercase();
        let completed = matches!(rs.status, RunStatus::Completed);
        let finished = rs.finished_at.map(|t| t.to_rfc3339());
        let entry = by_key.entry(key).or_default();
        if entry.last_run_id.is_none() {
            entry.last_run_id = Some(rs.run_id.clone());
            entry.last_run_status = Some(status);
            entry.last_run_finished_at = finished;
        }
        if completed {
            entry.ever_completed = true;
        }
    }
    Ok(by_key)
}

/// Stable key for matching list_plans paths ↔ run.json plan_path.
///
/// Prefer project-relative display string; fall back to absolute/lossy.
fn normalize_plan_key(project: &Path, plan_path: &Path) -> String {
    if plan_path.as_os_str().is_empty() {
        return String::new();
    }
    // Already relative under project (list_plans output).
    if plan_path.is_relative() {
        return plan_path.display().to_string();
    }
    if let Ok(rel) = plan_path.strip_prefix(project) {
        return rel.display().to_string();
    }
    // Canonicalize both sides when possible (symlinks / .. components).
    if let (Ok(proj), Ok(plan)) = (project.canonicalize(), plan_path.canonicalize()) {
        if let Ok(rel) = plan.strip_prefix(&proj) {
            return rel.display().to_string();
        }
        return plan.display().to_string();
    }
    plan_path.display().to_string()
}

/// Best-effort title without full plan load/validate (list must stay cheap).
/// G0: uses shared H1 sanitize (cut at ## / max 80 chars) so rail never shows full body.
fn plan_title_hint(project: &Path, rel: &str) -> Option<String> {
    let abs = if Path::new(rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        project.join(rel)
    };
    let text = std::fs::read_to_string(&abs).ok()?;
    // YAML / front-ish: name: foo
    for line in text.lines().take(40) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("name:") {
            let v = rest.trim().trim_matches('"').trim_matches('\'').trim();
            if !v.is_empty() {
                return Some(crate::services::chat::sanitize_plan_title(v));
            }
        }
    }
    // Markdown H1 (shared sanitize — handles single-line walls)
    if let Some(t) = crate::services::chat::extract_title_from_md(&text) {
        return Some(t);
    }
    abs.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
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
///
/// Always drops `optional && !include` before write/spawn (A0-R4 · D-T3-1),
/// same as [`crate::app::run::materialize_run`]. Mode B callers usually already
/// ran `materialize_selected_tasks` — re-apply is idempotent.
///
/// Route provenance: see [`start_run_from_plan_with_route`].
pub fn start_run_from_plan(config: Config, project: PathBuf, ir: &PlanIR) -> Result<String> {
    start_run_from_plan_with_route(config, project, ir, None)
}

/// Same as [`start_run_from_plan`] but stamps `route_source` from an optional
/// soft/force fill report (P1-2). When `route_report` is `None`, infers from IR.
pub fn start_run_from_plan_with_route(
    config: Config,
    project: PathBuf,
    ir: &PlanIR,
    route_report: Option<&crate::domain::worker::RouteFillReport>,
) -> Result<String> {
    // Single materialize path (Ensure closeout + checklist + optional drop + route stamp).
    let (run_id, run_state, ir, _cost_line) =
        crate::app::run::materialize_run_with_route(&config, project, ir, route_report)?;
    let run_dir = run_state.run_dir.clone();

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
    let poll_secs = config.default.poll_interval_secs.clamp(1, 30);
    let retry_max = config.default.retry_max;
    let stall_secs = config.default.stall_secs;
    let failover_enabled = config.default.failover_enabled;
    let fallback_extra_attempts = config.default.fallback_extra_attempts;
    let failover_order = config.default.failover_order.clone();
    let cost_escalate_enabled = config.default.cost_escalate_enabled;
    let browser_cfg = config.browser.clone();
    let config_for_ensure = config.clone();

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
                failover_enabled,
                fallback_extra_attempts,
                failover_order,
                cost_escalate_enabled,
                browser: browser_cfg,
                provider_unhealthy: Vec::new(),
            };
            match sched.run().await {
                Ok(status) => {
                    tracing::info!(%rid, ?status, "desktop run finished");
                    if let Ok(rs) = RunState::load(&runs_dir.join(&rid)) {
                        let _ = report::write_reports(&rs);
                    }
                    // Ensure E3 (thin IO hook → app; strategy not inlined here).
                    if matches!(status, RunStatus::Failed | RunStatus::Paused) {
                        let _ = crate::app::run::maybe_auto_rework_quiet(&config_for_ensure, &rid);
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
    get_plan_job, latest_plan_job_for_project, remove_proposed_task, sanitize_proposed_deps,
    start_plan_job, update_proposed_task, PlanJobView, SanitizeDepsResult, StartPlanJobRequest,
};

/// Freeze proposed plan and start scheduler; returns run_id.
///
/// A1 facade: business entry lives in [`crate::app::split::confirm`]; this
/// symbol stays for CLI/Tauri/tests until all call sites migrate.
pub fn confirm_start(config: Config, job_id: &str) -> Result<String> {
    crate::app::split::confirm(config, job_id, None)
}

pub fn stop_run(config: &Config, run_id: &str) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;
    let mut stopped: Vec<String> = Vec::new();
    for (tid, ts) in rs.tasks.iter_mut() {
        // Must include Pending: otherwise the in-process scheduler keeps
        // spawning later waves after the user hits "全部停止".
        if matches!(
            ts.status,
            TaskStatus::Running
                | TaskStatus::Starting
                | TaskStatus::Queued
                | TaskStatus::Pending
        ) {
            if let Some(pid) = ts.pid {
                kill_pid(pid);
            }
            // meta.json may hold a fresher pid than RunState (provider write race).
            let meta = dir.join("tasks").join(tid).join("meta.json");
            if meta.exists() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(
                    &std::fs::read_to_string(&meta).unwrap_or_default(),
                ) {
                    if let Some(pid) = v.get("pid").and_then(|p| p.as_u64()) {
                        kill_pid(pid as u32);
                    }
                }
            }
            let task_dir = dir.join("tasks").join(tid);
            let _ = std::fs::create_dir_all(&task_dir);
            let _ = std::fs::write(task_dir.join(".done"), "130");
            ts.status = TaskStatus::Stopped;
            ts.finished_at = Some(chrono::Utc::now());
            ts.pid = None;
            ts.exit_code = Some(130);
            ts.error = None; // not a business failure
            stopped.push(tid.clone());
        }
    }
    rs.status = RunStatus::Aborted;
    rs.finished_at = Some(chrono::Utc::now());
    rs.save()?;
    let _ = rs.event(
        "run_end",
        serde_json::json!({
            "status": "aborted",
            "via": "desktop",
            "stopped_tasks": stopped,
        }),
    );
    Ok(())
}

pub fn pause_run(config: &Config, run_id: &str) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;

    // Check if run is currently running
    if !matches!(rs.status, RunStatus::Running) {
        bail!("run 当前状态为 {:?},无法暂停", rs.status);
    }

    // Save current run state before pausing
    rs.save()?;

    rs.status = RunStatus::Paused;
    rs.save()?;

    let _ = rs.event(
        "run_pause",
        serde_json::json!({
            "status": "paused",
            "via": "desktop",
        }),
    );
    Ok(())
}

pub fn resume_run_async(config: Config, run_id: &str) -> Result<()> {
    spawn_resume(config, run_id, None)
}

/// Manual re-run of **one** failed/stopped/timeout task in an existing run dir.
///
/// Does **not** open a new Mode B plan or re-split. Done tasks stay Done; only
/// `task_id` is reset to Pending (fresh attempt budget) and the scheduler
/// continues from this run. Refuses while the run is still marked Running.
pub fn retry_task_async(config: Config, run_id: &str, task_id: &str) -> Result<()> {
    if task_id.trim().is_empty() {
        bail!("task_id 不能为空");
    }
    spawn_resume(config, run_id, Some(task_id.to_string()))
}

/// Shared background resume/retry spawn. `only_task = None` → whole-run resume
/// (all non-Done); `Some(id)` → single-task manual retry.
fn spawn_resume(config: Config, run_id: &str, only_task: Option<String>) -> Result<()> {
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
    if let Some(ref tid) = only_task {
        rs.prepare_task_retry(tid)?;
        let _ = rs.event(
            "task_retry",
            serde_json::json!({
                "task_id": tid,
                "reason": "manual",
                "via": "desktop",
            }),
        );
    } else {
        let _n = rs.prepare_for_resume();
    }
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
    let failover_enabled = config.default.failover_enabled;
    let fallback_extra_attempts = config.default.fallback_extra_attempts;
    let failover_order = config.default.failover_order.clone();
    let cost_escalate_enabled = config.default.cost_escalate_enabled;
    let browser_cfg = config.browser.clone();
    let config_for_ensure = config.clone();

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
                failover_enabled,
                fallback_extra_attempts,
                failover_order,
                cost_escalate_enabled,
                browser: browser_cfg,
                provider_unhealthy: Vec::new(),
            };
            let status = sched.run().await;
            if let Ok(st) = RunState::load(&runs_dir.join(&rid)) {
                let _ = report::write_reports(&st);
            }
            if let Ok(st) = status {
                if matches!(st, RunStatus::Failed | RunStatus::Paused) {
                    let _ = crate::app::run::maybe_auto_rework_quiet(&config_for_ensure, &rid);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn write_run_json(
        runs_dir: &Path,
        run_id: &str,
        project: &Path,
        plan_path: &Path,
        status: &str,
        finished: bool,
    ) {
        let dir = runs_dir.join(run_id);
        std::fs::create_dir_all(&dir).unwrap();
        let finished_field = if finished {
            format!(r#""finished_at": "{}","#, Utc::now().to_rfc3339())
        } else {
            String::new()
        };
        let body = format!(
            r#"{{
  "schema": "cco-run/v1",
  "run_id": "{run_id}",
  "project_root": "{project}",
  "plan_path": "{plan}",
  "adapter": "raw-single",
  "started_at": "{started}",
  {finished_field}
  "status": "{status}",
  "tasks": {{}}
}}"#,
            run_id = run_id,
            project = project.display(),
            plan = plan_path.display(),
            started = Utc::now().to_rfc3339(),
            finished_field = finished_field,
            status = status,
        );
        std::fs::write(dir.join("run.json"), body).unwrap();
    }

    #[test]
    fn list_plan_meta_ever_completed_from_run_status() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        let plans_dir = project.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let plan_done = plans_dir.join("done-plan.md");
        let plan_draft = plans_dir.join("draft-plan.md");
        std::fs::write(&plan_done, "# Done Plan\n\nbody\n").unwrap();
        std::fs::write(&plan_draft, "# Draft Plan\n\nbody\n").unwrap();

        let mut config = Config::default();
        config.state_root = tmp.path().join("state");
        std::fs::create_dir_all(config.runs_dir()).unwrap();
        let runs = config.runs_dir();

        // Older failed run + newer completed run for done-plan (absolute plan_path like real runs).
        write_run_json(
            &runs,
            "20260101T000000Z-aaaa",
            &project,
            &plan_done,
            "failed",
            true,
        );
        write_run_json(
            &runs,
            "20260102T000000Z-bbbb",
            &project,
            &plan_done,
            "completed",
            true,
        );
        // Unrelated project must not pollute.
        write_run_json(
            &runs,
            "20260103T000000Z-cccc",
            &tmp.path().join("other-proj"),
            &tmp.path().join("other-proj/plans/x.md"),
            "completed",
            true,
        );

        let metas = list_plan_meta(&config, &project).unwrap();
        let done = metas
            .iter()
            .find(|m| m.path == "plans/done-plan.md" || m.path.ends_with("done-plan.md"))
            .expect("done plan row");
        assert!(
            done.ever_completed,
            "completed run must set ever_completed=true: {done:?}"
        );
        assert_eq!(done.last_run_id.as_deref(), Some("20260102T000000Z-bbbb"));
        assert_eq!(done.last_run_status.as_deref(), Some("completed"));
        assert!(done.last_run_finished_at.is_some());
        assert_eq!(done.title.as_deref(), Some("Done Plan"));

        let draft = metas
            .iter()
            .find(|m| m.path == "plans/draft-plan.md" || m.path.ends_with("draft-plan.md"))
            .expect("draft plan row");
        assert!(
            !draft.ever_completed,
            "no run → ever_completed=false: {draft:?}"
        );
        assert!(draft.last_run_id.is_none());
        assert_eq!(draft.title.as_deref(), Some("Draft Plan"));
    }

    #[test]
    fn list_plan_meta_failed_only_is_not_ever_completed() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        let plans_dir = project.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let plan = plans_dir.join("failed-only.md");
        std::fs::write(&plan, "# Failed Only\n").unwrap();

        let mut config = Config::default();
        config.state_root = tmp.path().join("state");
        std::fs::create_dir_all(config.runs_dir()).unwrap();

        write_run_json(
            &config.runs_dir(),
            "20260104T000000Z-ffff",
            &project,
            &plan,
            "failed",
            true,
        );

        let metas = list_plan_meta(&config, &project).unwrap();
        let row = metas
            .iter()
            .find(|m| m.path.ends_with("failed-only.md"))
            .expect("row");
        assert!(!row.ever_completed);
        assert_eq!(row.last_run_status.as_deref(), Some("failed"));
    }

    #[test]
    fn normalize_plan_key_matches_relative_and_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("plans")).unwrap();
        let abs = project.join("plans/x.md");
        std::fs::write(&abs, "x").unwrap();

        let k_rel = normalize_plan_key(&project, Path::new("plans/x.md"));
        let k_abs = normalize_plan_key(&project, &abs);
        assert_eq!(k_rel, "plans/x.md");
        assert_eq!(k_abs, "plans/x.md");
    }
}
