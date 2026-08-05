//! Guide use cases — G0 shell: list / start / get guided sessions.
//!
//! [INPUT]: Config · project path · mode/entry · role pack id
//! [OUTPUT]: GuideSession (list/start/get); DTO = domain types
//! [POS]: Application 层；Presentation（Tauri/gateway）经本模块
//! [PROTOCOL]: 变更时更新此头部与 src/app/CLAUDE.md · docs/guided-plan-memory-decision-2026-07-21.md
//!
//! G0 = contract + shell: **no business policy** here yet (slot questions, brief synthesis,
//! role-pack content and materialize arrive with G1+). Never opens a run; `split::confirm`
//! stays the sole open-run entry.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::domain::guide::{GuideSession, SessionEntry, SessionMode};
use crate::state::guide_store;

/// List guided sessions for a project (newest first).
pub fn list(config: &Config, project: &Path) -> Result<Vec<GuideSession>> {
    let pid = project_id(project);
    guide_store::list_sessions(config, &pid)
}

/// Start a guided session (status `active`; slots/brief empty until G1).
pub fn start(
    config: &Config,
    project: &Path,
    mode: SessionMode,
    entry: SessionEntry,
    role_pack: &str,
) -> Result<GuideSession> {
    let pid = project_id(project);
    guide_store::start_session(config, &pid, mode, entry, role_pack)
}

/// Get one session by id (None when missing).
pub fn get(config: &Config, session_id: &str) -> Result<Option<GuideSession>> {
    guide_store::get_session(config, session_id)
}

fn project_id(project: &Path) -> String {
    project.to_string_lossy().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sqlite::reset_for_test;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn guide_shell_start_list_get() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        let project = PathBuf::from("/tmp/guide-app-proj");

        assert!(list(&cfg, &project).unwrap().is_empty());
        let s = start(
            &cfg,
            &project,
            SessionMode::Coop,
            SessionEntry::Socratic,
            "ship-product",
        )
        .unwrap();
        assert_eq!(s.role_pack, "ship-product");
        assert_eq!(s.mode, SessionMode::Coop);

        let got = get(&cfg, &s.session_id).unwrap().expect("found");
        assert_eq!(got, s);
        assert_eq!(list(&cfg, &project).unwrap().len(), 1);
        assert!(get(&cfg, "20200101T000000Z-nope").unwrap().is_none());
    }

    #[test]
    fn project_id_trims_trailing_slash() {
        assert_eq!(project_id(Path::new("/tmp/a/")), "/tmp/a");
        assert_eq!(project_id(Path::new("/tmp/a")), "/tmp/a");
    }
}
