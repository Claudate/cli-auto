//! Allowed-project list for desktop sidebar.
//!
//! [INPUT]: Config · project path
//! [OUTPUT]: ProjectSummary · list/add/remove_project
//! [POS]: services 子模块
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use crate::config::{AllowedProject, Config};

use super::runs::{list_runs, load_run};
use super::util::{is_live_task, paths_match};

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
    /// True when last_run_id was user-dismissed via「结束计划」(SQLite SoT).
    /// Sidebar must not treat it as「可续跑」current round.
    #[serde(default)]
    pub last_dismissed: bool,
    pub default_plan: Option<String>,
    pub last_plan: Option<String>,
}

/// List allowed projects with live run / CLI counts.
pub fn list_projects(config: &Config) -> Result<Vec<ProjectSummary>> {
    let runs = list_runs(config)?;
    let mut out = Vec::with_capacity(config.projects.len());
    for p in &config.projects {
        let path_str = p.path.display().to_string();
        let exists = p.path.is_dir();
        let for_proj: Vec<&super::runs::RunSummary> = runs
            .iter()
            .filter(|r| {
                let rp = PathBuf::from(&r.project_root);
                paths_match(&rp, &p.path)
            })
            .collect();
        // already newest-first from list_runs
        let last = for_proj.first().copied();
        let dismissed =
            crate::state::project_ui::try_get_dismissed_run_id(config, &p.path);
        let last_dismissed = match (last, dismissed.as_deref()) {
            (Some(l), Some(d)) if l.run_id == d => true,
            _ => false,
        };
        // Live only: do NOT promote an older `paused` run over a newer completed one.
        // Paused is a terminal-ish desk state of the *latest* run (stop_task left
        // pending siblings); surface it via last_status, not active_*.
        // Also never promote a dismissed terminal run as "active".
        let active = for_proj
            .iter()
            .find(|r| {
                matches!(
                    r.status.as_str(),
                    "running" | "validated" | "init" | "starting" | "queued" | "resuming"
                ) && dismissed
                    .as_deref()
                    .map(|d| d != r.run_id.as_str())
                    .unwrap_or(true)
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
            last_dismissed,
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
