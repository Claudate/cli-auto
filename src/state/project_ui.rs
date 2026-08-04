//! Per-project UI prefs in SQLite (durable · not process memory / not localStorage SoT).
//!
//! [INPUT]: Config · project path · key/value
//! [OUTPUT]: project_ui_prefs CRUD · dismissed_run_id helpers
//! [POS]: state adapter — SoT for desktop shell prefs that must survive reopen
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md
//!
//! Keys (convention):
//! - `dismissed_run_id` — user finished this run in UI; project_live must not re-bind it as current

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use crate::config::Config;

use super::sqlite::with_conn;

/// Key: last run the user dismissed via「结束计划」.
pub const KEY_DISMISSED_RUN_ID: &str = "dismissed_run_id";

fn project_id(project: &std::path::Path) -> String {
    project.display().to_string()
}

/// Set a UI pref (upsert).
pub fn set_pref(config: &Config, project: &std::path::Path, key: &str, value: &str) -> Result<()> {
    let pid = project_id(project);
    let now = Utc::now().to_rfc3339();
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO project_ui_prefs (project_id, key, value, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(project_id, key) DO UPDATE SET
                 value = excluded.value,
                 updated_at = excluded.updated_at"#,
            params![pid, key, value, now],
        )?;
        Ok(())
    })
}

/// Get a UI pref value.
pub fn get_pref(config: &Config, project: &std::path::Path, key: &str) -> Result<Option<String>> {
    let pid = project_id(project);
    with_conn(config, |conn| {
        let v: Option<String> = conn
            .query_row(
                "SELECT value FROM project_ui_prefs WHERE project_id = ?1 AND key = ?2",
                params![pid, key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(v)
    })
}

/// Delete a UI pref.
pub fn delete_pref(config: &Config, project: &std::path::Path, key: &str) -> Result<()> {
    let pid = project_id(project);
    with_conn(config, |conn| {
        conn.execute(
            "DELETE FROM project_ui_prefs WHERE project_id = ?1 AND key = ?2",
            params![pid, key],
        )?;
        Ok(())
    })
}

/// Mark a run as dismissed for this project (user ended the round in UI).
pub fn set_dismissed_run_id(
    config: &Config,
    project: &std::path::Path,
    run_id: &str,
) -> Result<()> {
    set_pref(config, project, KEY_DISMISSED_RUN_ID, run_id.trim())
}

pub fn get_dismissed_run_id(config: &Config, project: &std::path::Path) -> Result<Option<String>> {
    get_pref(config, project, KEY_DISMISSED_RUN_ID)
}

pub fn clear_dismissed_run_id(config: &Config, project: &std::path::Path) -> Result<()> {
    delete_pref(config, project, KEY_DISMISSED_RUN_ID)
}

/// Best-effort: never fail the UI path.
pub fn try_set_dismissed_run_id(config: &Config, project: &std::path::Path, run_id: &str) {
    if let Err(e) = set_dismissed_run_id(config, project, run_id) {
        tracing::warn!(
            error = %e,
            project = %project.display(),
            run_id = %run_id,
            "sqlite set dismissed_run_id failed"
        );
    }
}

pub fn try_get_dismissed_run_id(config: &Config, project: &std::path::Path) -> Option<String> {
    match get_dismissed_run_id(config, project) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                error = %e,
                project = %project.display(),
                "sqlite get dismissed_run_id failed"
            );
            None
        }
    }
}

pub fn try_clear_dismissed_run_id(config: &Config, project: &std::path::Path) {
    if let Err(e) = clear_dismissed_run_id(config, project) {
        tracing::warn!(
            error = %e,
            project = %project.display(),
            "sqlite clear dismissed_run_id failed"
        );
    }
}

/// True when this run was user-dismissed and must not bind as project live.
///
/// 「结束计划」soft-ends the desk even when status is `paused` (common after
/// stop_task left pending siblings). Only **actively executing** statuses
/// still surface so the user can stop a runaway CLI; paused/failed/done hide.
pub fn should_hide_run_as_current(dismissed: Option<&str>, run_id: &str, status: &str) -> bool {
    let Some(d) = dismissed else {
        return false;
    };
    if d.trim().is_empty() || d != run_id {
        return false;
    }
    let st = status.to_lowercase();
    // Keep only hard-live states visible after dismiss. `paused` is desk-state,
    // not an executing CLI — hide it once the user ended the round.
    !matches!(
        st.as_str(),
        "running" | "starting" | "queued" | "validated" | "init" | "resuming"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sqlite::reset_for_test;
    use tempfile::tempdir;

    fn test_cfg() -> (tempfile::TempDir, Config) {
        let dir = tempdir().unwrap();
        reset_for_test();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        (dir, cfg)
    }

    #[test]
    fn dismissed_run_roundtrip() {
        let (_dir, cfg) = test_cfg();
        let proj = std::path::Path::new("/tmp/proj-a");
        assert!(get_dismissed_run_id(&cfg, proj).unwrap().is_none());
        set_dismissed_run_id(&cfg, proj, "run-1").unwrap();
        assert_eq!(
            get_dismissed_run_id(&cfg, proj).unwrap().as_deref(),
            Some("run-1")
        );
        assert!(should_hide_run_as_current(Some("run-1"), "run-1", "failed"));
        // paused after 结束计划 must hide (stop_task residual desk)
        assert!(should_hide_run_as_current(Some("run-1"), "run-1", "paused"));
        assert!(should_hide_run_as_current(
            Some("run-1"),
            "run-1",
            "completed"
        ));
        assert!(!should_hide_run_as_current(
            Some("run-1"),
            "run-1",
            "running"
        ));
        assert!(!should_hide_run_as_current(
            Some("run-1"),
            "run-1",
            "starting"
        ));
        assert!(!should_hide_run_as_current(
            Some("run-1"),
            "run-2",
            "failed"
        ));
        assert!(!should_hide_run_as_current(
            Some("run-1"),
            "run-2",
            "paused"
        ));
        clear_dismissed_run_id(&cfg, proj).unwrap();
        assert!(get_dismissed_run_id(&cfg, proj).unwrap().is_none());
    }
}
