//! Project UI prefs use cases (SQLite SoT · dismissed run, etc.).
//!
//! [INPUT]: Config · project path · run_id
//! [OUTPUT]: dismiss / clear / get dismissed_run_id
//! [POS]: Application — Presentation 只调本模块；**禁止** UI 只写 localStorage
//! [PROTOCOL]: 变更时更新此头部与 src/app/CLAUDE.md

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::state::project_ui as store;

/// Persist「结束计划」for this project+run (survives app reopen).
pub fn dismiss_run(config: &Config, project: &Path, run_id: &str) -> Result<()> {
    let rid = run_id.trim();
    if rid.is_empty() {
        anyhow::bail!("empty run_id");
    }
    if !project.as_os_str().is_empty() {
        // ok
    }
    store::set_dismissed_run_id(config, project, rid)
}

/// Clear dismiss (e.g. user confirms a new run).
pub fn clear_dismissed_run(config: &Config, project: &Path) -> Result<()> {
    store::clear_dismissed_run_id(config, project)
}

/// Read dismissed run id if any.
pub fn get_dismissed_run(config: &Config, project: &Path) -> Result<Option<String>> {
    store::get_dismissed_run_id(config, project)
}

/// Best-effort dismiss (finish path never fails on storage).
pub fn try_dismiss_run(config: &Config, project: &Path, run_id: &str) {
    store::try_set_dismissed_run_id(config, project, run_id);
}

pub fn try_clear_dismissed_run(config: &Config, project: &Path) {
    store::try_clear_dismissed_run_id(config, project);
}

pub fn try_get_dismissed_run(config: &Config, project: &Path) -> Option<String> {
    store::try_get_dismissed_run_id(config, project)
}

/// Best-effort: stamp the project's current-run pointer at a run-start choke
/// point so project_live can `load_run` directly without scanning all history.
pub fn try_set_current_run(config: &Config, project: &Path, run_id: &str) {
    let rid = run_id.trim();
    if rid.is_empty() {
        return;
    }
    store::try_set_current_run_id(config, project, rid);
}

/// Read the current-run pointer if any.
pub fn try_get_current_run(config: &Config, project: &Path) -> Option<String> {
    store::try_get_current_run_id(config, project)
}

pub fn try_clear_current_run(config: &Config, project: &Path) {
    store::try_clear_current_run_id(config, project);
}
