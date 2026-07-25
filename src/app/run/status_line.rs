//! Application: compose [`StatusOneLiner`] from RunState / PlanJobView (H1).
//!
//! [INPUT]: RunState · PlanJobView · optional dual source
//! [OUTPUT]: StatusOneLiner DTO for CLI / live / TUI
//! [POS]: app/run · 不写策略到 live.rs 正文
//! [PROTOCOL]: 变更时更新此头部与 app/CLAUDE.md

use crate::domain::run::{
    from_plan_job, from_run, resolve_status_one_liner, PlanJobSnap, StatusOneLiner, TaskStatusSnap,
};
use crate::plan::planner::PlanJobView;
use crate::runtime::provider::TaskStatus;
use crate::state::{RunState, RunStatus};

/// One-liner from a loaded run (CLI `status` / finish summary path).
pub fn from_run_state(rs: &RunState) -> StatusOneLiner {
    let snaps = task_snaps(rs);
    let stall_any = false; // live may pass true; CLI status has no stall clock
    from_run(run_status_snake(rs.status), &snaps, stall_any)
}

/// One-liner from a plan job view (split desk / empty live).
pub fn from_job_view(job: &PlanJobView) -> StatusOneLiner {
    from_plan_job(&job_snap(job))
}

/// Dual-source for desktop live (H1-1 priority).
///
/// - Active run → run only
/// - Else job planning/planned → job
/// - Else terminal run → run
/// - Else idle
pub fn resolve(
    run: Option<&RunState>,
    job: Option<&PlanJobView>,
    stall_any: bool,
) -> StatusOneLiner {
    let run_arg = run.map(|rs| {
        let snaps = task_snaps(rs);
        (run_status_snake(rs.status), snaps, stall_any)
    });
    // resolve_status_one_liner needs slice refs with shared lifetime — rebuild
    match (run_arg, job) {
        (Some((st, snaps, stall)), j) => {
            resolve_status_one_liner(Some((st, &snaps, stall)), j.map(job_snap).as_ref())
        }
        (None, Some(j)) => from_job_view(j),
        (None, None) => StatusOneLiner::idle(),
    }
}

fn job_snap(job: &PlanJobView) -> PlanJobSnap {
    PlanJobSnap {
        status: job.status.clone(),
        task_count: job.task_count.map(|n| n as u32),
    }
}

fn task_snaps(rs: &RunState) -> Vec<TaskStatusSnap> {
    let mut ids: Vec<_> = rs.tasks.keys().cloned().collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| {
            let ts = rs.tasks.get(&id)?;
            // Prefer plan title from resolved if we only have id — title often absent on TaskState.
            let title = id.clone();
            Some(TaskStatusSnap {
                title,
                status: task_status_snake(ts.status).into(),
            })
        })
        .collect()
}

/// Prefer human titles when plan is available (optional enrichment).
pub fn from_run_state_with_titles(rs: &RunState, titles: &[(String, String)]) -> StatusOneLiner {
    let map: std::collections::HashMap<&str, &str> = titles
        .iter()
        .map(|(id, t)| (id.as_str(), t.as_str()))
        .collect();
    let mut ids: Vec<_> = rs.tasks.keys().cloned().collect();
    ids.sort();
    let snaps: Vec<TaskStatusSnap> = ids
        .into_iter()
        .filter_map(|id| {
            let ts = rs.tasks.get(&id)?;
            let title = map
                .get(id.as_str())
                .map(|s| (*s).to_string())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| id.clone());
            Some(TaskStatusSnap {
                title,
                status: task_status_snake(ts.status).into(),
            })
        })
        .collect();
    from_run(run_status_snake(rs.status), &snaps, false)
}

fn run_status_snake(s: RunStatus) -> &'static str {
    match s {
        RunStatus::Init => "init",
        RunStatus::Validated => "validated",
        RunStatus::Running => "running",
        RunStatus::Paused => "paused",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Aborted => "aborted",
    }
}

fn task_status_snake(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Queued => "queued",
        TaskStatus::Starting => "starting",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Timeout => "timeout",
        TaskStatus::Stopped => "stopped",
        TaskStatus::Skipped => "skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{RunState, TaskState};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn sample_run(status: RunStatus, tasks: Vec<(&str, TaskStatus)>) -> RunState {
        let mut map = HashMap::new();
        for (id, st) in tasks {
            map.insert(
                id.to_string(),
                TaskState {
                    status: st,
                    provider: "fake".into(),
                    mode: "print".into(),
                    session_id: None,
                    agent_id: None,
                    cost_usd: None,
                    exit_code: None,
                    error: None,
                    started_at: None,
                    finished_at: None,
                    work_dir: None,
                    worktree_branch: None,
                    pid: None,
                    terminals: vec![],
                    attempt: 1,
                    last_retry_reason: None,
                    failover_used: false,
                    failover_tried: vec![],
                    route_source: None,
                    route_previous: None,
                    route_note: None,
                },
            );
        }
        RunState {
            schema: "cco-run/v1".into(),
            run_id: "r1".into(),
            project_root: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            adapter: "test".into(),
            status,
            started_at: Utc::now(),
            finished_at: None,
            tasks: map,
            run_dir: PathBuf::from("/tmp/r1"),
        }
    }

    #[test]
    fn from_run_state_completed() {
        let rs = sample_run(
            RunStatus::Completed,
            vec![("a", TaskStatus::Done), ("b", TaskStatus::Done)],
        );
        let s = from_run_state(&rs);
        assert!(s.text.contains("已完成"));
        assert_eq!(s.done, 2);
        assert_eq!(s.total, 2);
    }

    #[test]
    fn titles_enrich_current_step() {
        let rs = sample_run(
            RunStatus::Running,
            vec![("t1", TaskStatus::Running), ("t2", TaskStatus::Pending)],
        );
        let titles = vec![("t1".into(), "实现登录".into())];
        let s = from_run_state_with_titles(&rs, &titles);
        assert!(s.text.contains("实现登录") || s.current_title.as_deref() == Some("实现登录"));
    }
}
