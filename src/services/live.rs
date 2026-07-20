//! Live multi-CLI views, task logs, external terminal, stop task.
//!
//! [INPUT]: Config · run_id · task_id · log_max_bytes
//! [OUTPUT]: ProjectLiveView · task_logs · open_task_terminal · stop_task
//! [POS]: services 子模块；桌面 monitor / LogConsole 主数据源
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use crate::config::Config;
use crate::plan::PlanIR;
use crate::runtime::handoff::{self, InspectLoopView};
use crate::runtime::log_events::{self, LogEvent};
use crate::runtime::provider::TaskStatus;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

use super::runs::{list_runs, load_run, RunSummary};
use super::util::{
    compact_log_tail_for_live, kill_pid, paths_match, read_log_tail, status_str, task_status_str,
};

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
    /// How many times this task has been started (1 = first try).
    #[serde(default)]
    pub attempt: u32,
    /// Last auto-retry reason (stall / fail / timeout), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_retry_reason: Option<String>,
    /// Seconds since stdout last grew (live tasks only; H3 stall strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stall_idle_secs: Option<u64>,
    /// Config stall threshold (seconds) for UI copy; always set when config known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stall_threshold_secs: Option<u64>,
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
    /// Mode B planner spend (USD), if this run was confirmed from a plan job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_cost_usd: Option<f64>,
    /// Sum of worker task costs (USD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_cost_usd: Option<f64>,
    /// P-loop: inspect VERDICT / blocking count / rework eligibility (desktop strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspect_loop: Option<InspectLoopView>,
    /// Run directory on disk (for open handoff / report).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_dir: Option<String>,
    /// Absolute path to host handoff.md when present (multi-cli P2-6 Board).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_md_path: Option<String>,
    /// Compact Board rows from handoff.json (id/provider/role/status).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handoff_board: Vec<HandoffBoardRowView>,
}

/// Compact Board row for desktop handoff strip (multi-cli P2-6).
#[derive(Debug, Clone, Serialize)]
pub struct HandoffBoardRowView {
    pub id: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub status: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

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
            planner_cost_usd: None,
            exec_cost_usd: None,
            inspect_loop: None,
            run_dir: None,
            handoff_md_path: None,
            handoff_board: vec![],
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
            // Live payload: shrink raw tail when events exist (P1-1 减负).
            let log_tail = compact_log_tail_for_live(&log_tail, !log_events.is_empty(), 6_000);
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
            let stall_threshold_secs = Some(config.default.stall_secs);
            let stall_idle_secs = stall_idle_secs_for(&stdout, ts);
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
                attempt: ts.attempt,
                last_retry_reason: ts.last_retry_reason.clone(),
                stall_idle_secs,
                stall_threshold_secs,
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

    let planner_cost_usd = crate::plan::planner::planner_cost_for_run(&rs.run_dir);
    let mut exec_sum = 0.0;
    let mut has_exec = false;
    for t in &tasks {
        if let Some(c) = t.cost_usd {
            exec_sum += c;
            has_exec = true;
        }
    }
    let exec_cost_usd = if has_exec { Some(exec_sum) } else { None };

    let inspect_loop = Some(handoff::inspect_loop_view(
        resolved.as_ref(),
        &rs,
        &rs.project_root,
    ));

    // multi-cli P2-6: expose handoff Board for desktop strip / open ledger.
    let handoff_md_path = {
        let p = handoff::Handoff::path_md(&rs.run_dir);
        if p.is_file() {
            Some(p.display().to_string())
        } else {
            None
        }
    };
    let handoff_board = handoff::Handoff::load(&rs.run_dir)
        .map(|h| {
            h.board
                .into_iter()
                .map(|r| HandoffBoardRowView {
                    id: r.id,
                    provider: r.provider,
                    role: r.role,
                    status: r.status,
                    scope: r.scope,
                    cost: r.cost,
                })
                .collect()
        })
        .unwrap_or_default();

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
        planner_cost_usd,
        exec_cost_usd,
        inspect_loop,
        run_dir: Some(rs.run_dir.display().to_string()),
        handoff_md_path,
        handoff_board,
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

/// Open external (or embedded) terminal following a task's stdout/stderr (P1-2).
pub fn open_task_terminal(
    config: &Config,
    run_id: &str,
    task_id: &str,
    kind: Option<&str>,
) -> Result<crate::terminal::TerminalSession> {
    let rs = load_run(config, run_id)?;
    if !rs.tasks.contains_key(task_id) {
        bail!("unknown task: {task_id}");
    }
    let task_dir = rs.task_dir(task_id);
    let mut cwd = rs.project_root.clone();
    if let Some(ts) = rs.tasks.get(task_id) {
        if let Some(p) = &ts.work_dir {
            cwd = p.clone();
        }
    }
    let wd = task_dir.join("work_dir.json");
    if wd.exists() {
        if let Ok(text) = std::fs::read_to_string(&wd) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(p) = v.get("work_dir").and_then(|x| x.as_str()) {
                    cwd = PathBuf::from(p);
                }
            }
        }
    }
    let stdout = task_dir.join("stdout.json");
    let stderr = task_dir.join("stderr.log");
    // Ensure files exist so `tail -f` works immediately.
    if !stdout.exists() {
        let _ = std::fs::create_dir_all(&task_dir);
        let _ = std::fs::write(&stdout, "");
    }
    if !stderr.exists() {
        let _ = std::fs::write(&stderr, "");
    }

    let kind = match kind.unwrap_or("external").to_ascii_lowercase().as_str() {
        "embedded" => SessionKind::Embedded,
        _ => SessionKind::External,
    };

    let tm = TerminalManager::for_run(
        &rs.run_dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    )
    .with_limits(config.terminal.max_embedded, config.terminal.max_external);

    let session = tm.open_follow_logs(task_id, &cwd, &stdout, &stderr, kind)?;

    // Persist session id on task state (best-effort).
    let mut rs = rs;
    if let Some(ts) = rs.tasks.get_mut(task_id) {
        if !ts.terminals.iter().any(|id| id == &session.id) {
            ts.terminals.push(session.id.clone());
        }
    }
    let _ = rs.save();
    let _ = rs.event(
        "terminal_open",
        serde_json::json!({
            "task_id": task_id,
            "kind": session.kind,
            "session_id": session.id,
            "launcher": session.launcher,
            "via": "desktop",
        }),
    );
    Ok(session)
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
                TaskStatus::Running
                    | TaskStatus::Starting
                    | TaskStatus::Queued
                    | TaskStatus::Pending
            ) {
                if let Some(pid) = ts.pid {
                    kill_pid(pid);
                }
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
            }
        }
    }

    // Whole-run stop (no task_id) → Aborted so the scheduler loop exits and
    // never spawns remaining pending work. Single-task stop keeps pending
    // siblings so the user can keep the rest of the graph.
    let still_live = rs.tasks.values().any(|t| {
        matches!(
            t.status,
            TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
        )
    });
    let still_pending = rs.tasks.values().any(|t| t.status == TaskStatus::Pending);
    if task_id.is_none() {
        // stop_all path: freeze every remaining pending too.
        for (_tid, ts) in rs.tasks.iter_mut() {
            if ts.status == TaskStatus::Pending {
                ts.status = TaskStatus::Stopped;
                ts.finished_at = Some(chrono::Utc::now());
            }
        }
        rs.status = RunStatus::Aborted;
        rs.finished_at = Some(chrono::Utc::now());
    } else if !still_live && !still_pending {
        rs.status = RunStatus::Aborted;
        rs.finished_at = Some(chrono::Utc::now());
    } else if !still_live {
        // All stopped workers, siblings still pending — pause so scheduler
        // reloads Aborted/Paused from disk and does not keep spawning.
        rs.status = RunStatus::Paused;
        rs.finished_at = Some(chrono::Utc::now());
    }
    rs.save()?;
    let _ = rs.event(
        "task_stop",
        serde_json::json!({
            "tasks": targets,
            "via": "desktop",
            "run_status": match rs.status {
                RunStatus::Aborted => "aborted",
                RunStatus::Paused => "paused",
                _ => "running",
            },
        }),
    );
    Ok(())
}

/// Approx seconds since stdout last grew — for H3 stall strip (not the patrol clock).
/// Uses file mtime; falls back to time since `started_at` when the file is missing.
fn stall_idle_secs_for(stdout: &Path, ts: &state::TaskState) -> Option<u64> {
    if !matches!(
        ts.status,
        TaskStatus::Running | TaskStatus::Starting
    ) {
        return None;
    }
    let now = std::time::SystemTime::now();
    if let Ok(meta) = std::fs::metadata(stdout) {
        if let Ok(modified) = meta.modified() {
            return Some(now.duration_since(modified).unwrap_or_default().as_secs());
        }
    }
    ts.started_at.map(|s| {
        chrono::Utc::now()
            .signed_duration_since(s)
            .num_seconds()
            .max(0) as u64
    })
}
