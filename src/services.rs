//! Shared backend calls used by the native GUI (same logic as CLI).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{AllowedProject, Config};
use crate::doctor::{self, DoctorReport};
use crate::plan::{self, load_plan, PlanIR};
use crate::report;
use crate::runtime::log_events::{self, LogEvent};
use crate::runtime::provider::{ProviderRegistry, TaskStatus};
use crate::runtime::Scheduler;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

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

/// Project row for the desktop sidebar (allowed list, not a filesystem tree).
#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub active_run_id: Option<String>,
    pub active_status: Option<String>,
    pub running_tasks: usize,
    pub total_tasks: usize,
    pub last_run_id: Option<String>,
    pub last_status: Option<String>,
    pub default_plan: Option<String>,
    pub last_plan: Option<String>,
}

/// One CLI worker / task for live multi-CLI view.
#[derive(Debug, Clone, Serialize)]
pub struct TaskLiveView {
    pub task_id: String,
    pub title: Option<String>,
    pub status: String,
    pub provider: String,
    pub mode: String,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub work_dir: Option<String>,
    pub log_tail: String,
    pub log_bytes: u64,
    /// Structured events for desktop readable console (tail window).
    #[serde(default)]
    pub log_events: Vec<LogEvent>,
    /// One-line human error summary when failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    /// From plan.resolved.json when available.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Deps not yet done (for pending/queued display).
    #[serde(default)]
    pub waiting_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectLiveView {
    pub project_path: String,
    pub project_name: String,
    pub run_id: Option<String>,
    pub run_status: Option<String>,
    pub plan_path: Option<String>,
    pub started_at: Option<String>,
    pub tasks: Vec<TaskLiveView>,
    /// Topo layers from resolved plan (wave display).
    #[serde(default)]
    pub layers: Vec<Vec<String>>,
    /// 1-based current wave index (first layer with non-terminal tasks), if any.
    #[serde(default)]
    pub current_wave: Option<usize>,
    pub max_parallel: Option<usize>,
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

/// Subset of config exposed to the desktop UI for reading.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub poll_interval_secs: u64,
    pub default_provider: String,
    pub default_mode: String,
    pub max_parallel: usize,
    pub ui_refresh_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsUpdate {
    pub poll_interval_secs: Option<u64>,
    pub default_provider: Option<String>,
    pub default_mode: Option<u32>,
    pub max_parallel: Option<usize>,
}

pub fn get_settings(config: &Config) -> SettingsView {
    SettingsView {
        poll_interval_secs: config.default.poll_interval_secs,
        default_provider: config.default.default_provider.clone(),
        default_mode: config.default.default_mode.clone(),
        max_parallel: config.default.max_parallel,
        ui_refresh_secs: 2, // UI hardcoded; could become configurable later
    }
}

/// Apply partial update to config and persist.
pub fn set_settings(config: &mut Config, update: SettingsUpdate) -> Result<()> {
    if let Some(v) = update.poll_interval_secs {
        config.default.poll_interval_secs = v.clamp(1, 60);
    }
    if let Some(p) = update.default_provider {
        if !p.is_empty() {
            config.default.default_provider = p;
        }
    }
    if let Some(m) = update.default_mode {
        match m {
            0 => config.default.default_mode = "print".to_string(),
            1 => config.default.default_mode = "bg".to_string(),
            2 => config.default.default_mode = "auto".to_string(),
            _ => {}
        }
    }
    if let Some(v) = update.max_parallel {
        config.default.max_parallel = v.clamp(1, 32);
    }
    config.save()
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

pub use crate::plan::planner::{get_plan_job, latest_plan_job_for_project, start_plan_job, PlanJobView, StartPlanJobRequest};

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
        use crate::runtime::provider::TaskStatus;
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
        if matches!(ts.status, crate::runtime::provider::TaskStatus::Pending) {
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
                poll_interval: Duration::from_secs(
                    config.default.poll_interval_secs.clamp(1, 30),
                ),
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
            };
            let _ = sched.run().await;
            if let Ok(st) = RunState::load(&runs_dir.join(&rid)) {
                let _ = report::write_reports(&st);
            }
        });
    });
    Ok(())
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => a.to_string_lossy() == b.to_string_lossy(),
    }
}

fn status_str(s: &RunStatus) -> String {
    format!("{s:?}").to_ascii_lowercase()
}

fn task_status_str(s: &TaskStatus) -> String {
    format!("{s:?}").to_ascii_lowercase()
}

fn is_live_task(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
    )
}

fn read_log_tail(path: &Path, max_bytes: usize) -> (String, u64) {
    let meta_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if meta_len == 0 {
        return (String::new(), 0);
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(max_bytes);
            let slice = &bytes[start..];
            // Prefer UTF-8; fall back to lossy.
            let mut text = String::from_utf8_lossy(slice).into_owned();
            if start > 0 {
                // Drop partial first line when we sliced mid-file.
                if let Some(pos) = text.find('\n') {
                    text = text[pos + 1..].to_string();
                }
                text = format!("… (truncated, {} bytes total)\n{}", meta_len, text);
            }
            (text, meta_len)
        }
        Err(_) => (String::new(), meta_len),
    }
}

/// List allowed projects with live run / CLI counts.
pub fn list_projects(config: &Config) -> Result<Vec<ProjectSummary>> {
    let runs = list_runs(config)?;
    let mut out = Vec::with_capacity(config.projects.len());
    for p in &config.projects {
        let path_str = p.path.display().to_string();
        let exists = p.path.is_dir();
        let for_proj: Vec<&RunSummary> = runs
            .iter()
            .filter(|r| {
                let rp = PathBuf::from(&r.project_root);
                paths_match(&rp, &p.path)
            })
            .collect();
        // already newest-first from list_runs
        let last = for_proj.first().copied();
        let active = for_proj
            .iter()
            .find(|r| {
                matches!(
                    r.status.as_str(),
                    "running" | "validated" | "init" | "paused"
                )
            })
            .copied();

        let (running_tasks, total_tasks) = if let Some(a) = active {
            if let Ok(rs) = load_run(config, &a.run_id) {
                let running = rs
                    .tasks
                    .values()
                    .filter(|t| is_live_task(&t.status))
                    .count();
                (running, rs.tasks.len())
            } else {
                (0, a.task_count)
            }
        } else {
            (0, last.map(|l| l.task_count).unwrap_or(0))
        };

        out.push(ProjectSummary {
            path: path_str,
            name: p.display_name(),
            exists,
            active_run_id: active.map(|a| a.run_id.clone()),
            active_status: active.map(|a| a.status.clone()),
            running_tasks,
            total_tasks,
            last_run_id: last.map(|l| l.run_id.clone()),
            last_status: last.map(|l| l.status.clone()),
            default_plan: p.default_plan.as_ref().map(|s| s.display().to_string()),
            last_plan: p.last_plan.as_ref().map(|s| s.display().to_string()),
        });
    }
    Ok(out)
}

pub fn add_project(config: &mut Config, path: PathBuf, name: Option<String>) -> Result<AllowedProject> {
    if !path.is_dir() {
        bail!("不是有效目录: {}", path.display());
    }
    config.add_project(path, name)
}

pub fn remove_project(config: &mut Config, path: &Path) -> Result<bool> {
    config.remove_project(path)
}

/// Live multi-CLI view for a project: active (or latest) run + per-task log tails.
pub fn project_live_view(
    config: &Config,
    project: &Path,
    log_max_bytes: usize,
) -> Result<ProjectLiveView> {
    let name = project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    // Prefer matching allowed-project display name.
    let name = config
        .projects
        .iter()
        .find(|p| paths_match(&p.path, project))
        .map(|p| p.display_name())
        .unwrap_or(name);

    let runs = list_runs(config)?;
    let for_proj: Vec<&RunSummary> = runs
        .iter()
        .filter(|r| paths_match(Path::new(&r.project_root), project))
        .collect();
    let chosen = for_proj
        .iter()
        .find(|r| {
            matches!(
                r.status.as_str(),
                "running" | "validated" | "init" | "paused"
            )
        })
        .or_else(|| for_proj.first())
        .copied();

    let Some(sum) = chosen else {
        return Ok(ProjectLiveView {
            project_path: project.display().to_string(),
            project_name: name,
            run_id: None,
            run_status: None,
            plan_path: None,
            started_at: None,
            tasks: vec![],
            layers: vec![],
            current_wave: None,
            max_parallel: None,
        });
    };

    let rs = load_run(config, &sum.run_id)?;
    // Resolved plan for titles / depends / waves
    let resolved_path = rs.run_dir.join("plan.resolved.json");
    let resolved: Option<PlanIR> = std::fs::read_to_string(&resolved_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let layers = resolved
        .as_ref()
        .map(crate::graph::topo_layers)
        .unwrap_or_default();
    let max_parallel = resolved.as_ref().map(|p| p.max_parallel);
    let done_ids: HashSet<String> = rs
        .tasks
        .iter()
        .filter(|(_, ts)| {
            matches!(
                ts.status,
                TaskStatus::Done | TaskStatus::Skipped
            )
        })
        .map(|(id, _)| id.clone())
        .collect();

    let mut tasks: Vec<TaskLiveView> = rs
        .tasks
        .iter()
        .map(|(tid, ts)| {
            let stdout = rs.task_dir(tid).join("stdout.json");
            let stderr = rs.task_dir(tid).join("stderr.log");
            // Prefer stdout (JSONL / result); append stderr if present.
            // Prefer a large stdout window so transcript keeps tool/assistant lines.
            let stdout_budget = log_max_bytes.max(96_000);
            let (stdout_tail, log_bytes) = if stdout.exists() {
                read_log_tail(&stdout, stdout_budget)
            } else {
                (String::new(), 0)
            };
            // stderr: small tail for raw; parser will collapse to one summary event.
            let stderr_tail = if stderr.exists() {
                read_log_tail(&stderr, 12_000.min(log_max_bytes / 4).max(4_000)).0
            } else {
                String::new()
            };
            let mut log_tail = stdout_tail.clone();
            if !stderr_tail.is_empty() {
                if !log_tail.is_empty() {
                    log_tail.push_str("\n--- stderr ---\n");
                }
                log_tail.push_str(&stderr_tail);
            }
            // Structured events (stderr folded to 1 row).
            let log_events = log_events::parse_worker_logs(&stdout_tail, &stderr_tail, 300);
            let error_summary = log_events::error_summary_from(&log_events, ts.error.as_deref());
            let (title, depends_on) = resolved
                .as_ref()
                .and_then(|p| p.task(tid))
                .map(|t| (Some(t.title.clone()), t.depends_on.clone()))
                .unwrap_or((None, vec![]));
            let waiting_on: Vec<String> = depends_on
                .iter()
                .filter(|d| !done_ids.contains(*d))
                .cloned()
                .collect();
            TaskLiveView {
                task_id: tid.clone(),
                title,
                status: task_status_str(&ts.status),
                provider: ts.provider.clone(),
                mode: ts.mode.clone(),
                cost_usd: ts.cost_usd,
                session_id: ts.session_id.clone(),
                agent_id: ts.agent_id.clone(),
                pid: ts.pid,
                error: ts.error.clone(),
                started_at: ts.started_at.map(|t| t.to_rfc3339()),
                finished_at: ts.finished_at.map(|t| t.to_rfc3339()),
                work_dir: ts.work_dir.as_ref().map(|p| p.display().to_string()),
                log_tail,
                log_bytes,
                log_events,
                error_summary,
                depends_on,
                waiting_on,
            }
        })
        .collect();
    // Live / running first, then by task_id.
    tasks.sort_by(|a, b| {
        let rank = |s: &str| match s {
            "running" | "starting" => 0,
            "queued" => 1,
            "pending" => 2,
            "paused" => 3,
            "done" | "completed" => 4,
            _ => 5,
        };
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });

    // Current wave: first layer that still has a non-terminal task.
    let current_wave = if layers.is_empty() {
        None
    } else {
        let mut cw = None;
        for (i, layer) in layers.iter().enumerate() {
            let any_open = layer.iter().any(|id| {
                rs.tasks.get(id).map(|t| !t.status.is_terminal()).unwrap_or(false)
            });
            if any_open {
                cw = Some(i + 1);
                break;
            }
        }
        // All terminal → last wave number
        cw.or_else(|| Some(layers.len()))
    };

    Ok(ProjectLiveView {
        project_path: project.display().to_string(),
        project_name: name,
        run_id: Some(rs.run_id.clone()),
        run_status: Some(status_str(&rs.status)),
        plan_path: Some(rs.plan_path.display().to_string()),
        started_at: Some(rs.started_at.to_rfc3339()),
        tasks,
        layers,
        current_wave,
        max_parallel,
    })
}

/// Full log payload for one task (raw + structured events).
#[derive(Debug, Clone, Serialize)]
pub struct TaskLogsView {
    pub text: String,
    pub bytes: u64,
    pub events: Vec<LogEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
}

/// Tail logs for one task (stdout + optional stderr) and parse events.
pub fn task_logs(
    config: &Config,
    run_id: &str,
    task_id: &str,
    max_bytes: usize,
) -> Result<TaskLogsView> {
    let rs = load_run(config, run_id)?;
    if !rs.tasks.contains_key(task_id) {
        bail!("unknown task: {task_id}");
    }
    let stdout = rs.task_dir(task_id).join("stdout.json");
    let stderr = rs.task_dir(task_id).join("stderr.log");
    let (stdout_tail, bytes) = if stdout.exists() {
        read_log_tail(&stdout, max_bytes)
    } else {
        (String::new(), 0)
    };
    let stderr_tail = if stderr.exists() {
        read_log_tail(&stderr, max_bytes / 2).0
    } else {
        String::new()
    };
    let mut text = stdout_tail.clone();
    if !stderr_tail.is_empty() {
        if !text.is_empty() {
            text.push_str("\n--- stderr ---\n");
        }
        text.push_str(&stderr_tail);
    }
    let events = log_events::parse_worker_logs(&stdout_tail, &stderr_tail, 400);
    let err_fallback = rs.tasks.get(task_id).and_then(|t| t.error.as_deref());
    let error_summary = log_events::error_summary_from(&events, err_fallback);
    Ok(TaskLogsView {
        text,
        bytes,
        events,
        error_summary,
    })
}

/// Stop a single task (or whole run if task_id is None).
pub fn stop_task(config: &Config, run_id: &str, task_id: Option<&str>) -> Result<()> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;
    let targets: Vec<String> = match task_id {
        Some(tid) => {
            if !rs.tasks.contains_key(tid) {
                bail!("unknown task: {tid}");
            }
            vec![tid.to_string()]
        }
        None => rs
            .tasks
            .iter()
            .filter(|(_, ts)| {
                matches!(
                    ts.status,
                    TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
                )
            })
            .map(|(id, _)| id.clone())
            .collect(),
    };

    for tid in &targets {
        if let Some(ts) = rs.tasks.get_mut(tid) {
            if matches!(
                ts.status,
                TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued | TaskStatus::Pending
            ) {
                if let Some(pid) = ts.pid {
                    kill_pid(pid);
                }
                let task_dir = dir.join("tasks").join(tid);
                let _ = std::fs::create_dir_all(&task_dir);
                let _ = std::fs::write(task_dir.join(".done"), "130");
                ts.status = TaskStatus::Stopped;
                ts.finished_at = Some(chrono::Utc::now());
            }
        }
    }

    // If any tasks still live, keep run running; else abort.
    let still_live = rs.tasks.values().any(|t| {
        matches!(
            t.status,
            TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
        )
    });
    let still_pending = rs.tasks.values().any(|t| t.status == TaskStatus::Pending);
    if !still_live && !still_pending {
        rs.status = RunStatus::Aborted;
        rs.finished_at = Some(chrono::Utc::now());
    } else if !still_live {
        // All stopped but some pending — pause so scheduler can exit cleanly if needed.
        rs.status = RunStatus::Paused;
    }
    rs.save()?;
    let _ = rs.event(
        "task_stop",
        serde_json::json!({
            "tasks": targets,
            "via": "desktop",
        }),
    );
    Ok(())
}
