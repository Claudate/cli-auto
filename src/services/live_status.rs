//! Live status_one_liner assembly (H1 · keep live.rs from growing).
//!
//! [INPUT]: Config · project · RunState · TaskLiveView tails
//! [OUTPUT]: Option<String> human sentence
//! [POS]: services 薄辅助；规则在 domain/app status_line
//! [PROTOCOL]: 变更时更新此头部

use std::path::Path;

use crate::config::Config;
use crate::state::RunState;

use super::live::TaskLiveView;

/// Compose `status_one_liner` for a loaded run (stall-aware when active).
pub fn compose_for_run(
    config: &Config,
    project: &Path,
    rs: &RunState,
    tasks: &[TaskLiveView],
) -> Option<String> {
    let stall_any = tasks.iter().any(|t| {
        t.stall_idle_secs
            .zip(t.stall_threshold_secs)
            .is_some_and(|(idle, thr)| idle >= thr)
    });
    let rs_st = super::util::status_str(&rs.status);
    if matches!(rs_st.as_str(), "completed" | "failed" | "aborted") {
        return latest_job_line(config, project).or_else(|| {
            let titles: Vec<(String, String)> = tasks
                .iter()
                .filter_map(|t| {
                    t.title
                        .as_ref()
                        .map(|title| (t.task_id.clone(), title.clone()))
                })
                .collect();
            Some(crate::app::run::status_line::from_run_state_with_titles(rs, &titles).text)
        });
    }
    let snaps: Vec<_> = tasks
        .iter()
        .map(|t| crate::domain::run::TaskStatusSnap {
            title: t
                .title
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| t.task_id.clone()),
            status: t.status.clone(),
        })
        .collect();
    Some(crate::domain::run::from_run(rs_st.as_str(), &snaps, stall_any).text)
}

/// Best-effort plan-job one-liner when live has no active run.
pub fn latest_job_line(config: &Config, project: &Path) -> Option<String> {
    let job = crate::app::split::latest_job_for_project(config, project).ok()??;
    if matches!(
        job.status.as_str(),
        "planning" | "planned" | "plan_failed" | "confirmed" | "ready"
    ) {
        Some(crate::app::run::status_line::from_job_view(&job).text)
    } else {
        None
    }
}
