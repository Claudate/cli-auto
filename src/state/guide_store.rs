//! Guide store: user_profile + rich project_memory + guide_* tables (G0-2 · shell).
//!
//! [INPUT]: Config · GuideSession · UserProfileRow · ProjectMemoryRow
//! [OUTPUT]: ~/.cco/cco.db — user_profile / project_memory / guide_sessions / guide_rounds / guide_utterances
//! [POS]: state adapter — SQLite SoT for guided sessions
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md · docs/guided-plan-memory-decision-2026-07-21.md
//!
//! G0 = contract + shell: full schema created, session CRUD (start/get/list) +
//! profile/memory get/upsert. Round/utterance **reads/writes arrive with G2**
//! (human-gated rounds); schema only here. Reuses (never duplicates) the P2-2 thin
//! tables `project_last_summary` / `project_pins` — do not dual-write them.

use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::config::Config;
use crate::domain::guide::{GuideBrief, GuideSession, SessionEntry, SessionMode, SessionStatus};
use crate::state::new_session_id;

use super::sqlite::with_conn;

/// `user_profile` row (profile_id defaults to `local`; weak cross-project profile).
#[derive(Debug, Clone, Serialize)]
pub struct UserProfileRow {
    pub profile_id: String,
    pub display_name: Option<String>,
    pub prefs_json: String,
    pub traits_json: String,
    pub updated_at: String,
}

/// Rich `project_memory` row (G0 shell: free-form JSON columns; G1 reads them).
#[derive(Debug, Clone, Serialize)]
pub struct ProjectMemoryRow {
    pub project: String,
    pub summary: String,
    pub open_tensions_json: String,
    pub last_role_pack: Option<String>,
    pub last_brief_json: Option<String>,
    pub signals_json: String,
    pub updated_at: String,
}

pub(crate) fn ensure_guide_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Guide G0: user profile / rich project memory / guided sessions (docs §5.6.1 target shape)
        CREATE TABLE IF NOT EXISTS user_profile (
          profile_id TEXT PRIMARY KEY,
          display_name TEXT,
          prefs_json TEXT NOT NULL DEFAULT '{}',
          traits_json TEXT NOT NULL DEFAULT '{}',
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS project_memory (
          project TEXT PRIMARY KEY,
          summary TEXT NOT NULL DEFAULT '',
          open_tensions_json TEXT NOT NULL DEFAULT '[]',
          last_role_pack TEXT,
          last_brief_json TEXT,
          signals_json TEXT NOT NULL DEFAULT '{}',
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS guide_sessions (
          session_id TEXT PRIMARY KEY,
          project TEXT NOT NULL,
          mode TEXT NOT NULL,
          entry TEXT NOT NULL,
          status TEXT NOT NULL,
          role_pack TEXT NOT NULL,
          slots_json TEXT NOT NULL DEFAULT '{}',
          brief_json TEXT,
          plan_path TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_guide_sessions_project
          ON guide_sessions(project, updated_at DESC);
        CREATE TABLE IF NOT EXISTS guide_rounds (
          session_id TEXT NOT NULL,
          round_idx INTEGER NOT NULL,
          checkpoint_kind TEXT NOT NULL,
          human_verdict TEXT,
          host_scores_json TEXT,
          intervention TEXT,
          summary TEXT,
          created_at TEXT NOT NULL,
          PRIMARY KEY (session_id, round_idx)
        );
        CREATE TABLE IF NOT EXISTS guide_utterances (
          session_id TEXT NOT NULL,
          round_idx INTEGER NOT NULL,
          seq INTEGER NOT NULL,
          speaker_id TEXT NOT NULL,
          role_label TEXT,
          need_tag TEXT,
          content TEXT NOT NULL,
          created_at TEXT NOT NULL,
          PRIMARY KEY (session_id, round_idx, seq)
        );
        "#,
    )?;
    Ok(())
}

/// Get the local user profile (creates a default row when missing).
pub fn get_user_profile(config: &Config) -> Result<UserProfileRow> {
    with_conn(config, |conn| {
        let row = conn
            .query_row(
                "SELECT profile_id, display_name, prefs_json, traits_json, updated_at
                 FROM user_profile WHERE profile_id = 'local'",
                [],
                |r| {
                    Ok(UserProfileRow {
                        profile_id: r.get(0)?,
                        display_name: r.get(1)?,
                        prefs_json: r.get(2)?,
                        traits_json: r.get(3)?,
                        updated_at: r.get(4)?,
                    })
                },
            )
            .optional()?;
        if let Some(row) = row {
            return Ok(row);
        }
        let row = UserProfileRow {
            profile_id: "local".into(),
            display_name: None,
            prefs_json: "{}".into(),
            traits_json: "{}".into(),
            updated_at: Utc::now().to_rfc3339(),
        };
        conn.execute(
            r#"INSERT INTO user_profile (profile_id, display_name, prefs_json, traits_json, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![row.profile_id, row.display_name, row.prefs_json, row.traits_json, row.updated_at],
        )?;
        Ok(row)
    })
}

/// Upsert the local user profile (best-effort shell; G1+ feeds traits).
pub fn upsert_user_profile(
    config: &Config,
    display_name: Option<&str>,
    prefs_json: &str,
    traits_json: &str,
) -> Result<UserProfileRow> {
    let updated_at = Utc::now().to_rfc3339();
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO user_profile (profile_id, display_name, prefs_json, traits_json, updated_at)
               VALUES ('local', ?1, ?2, ?3, ?4)
               ON CONFLICT(profile_id) DO UPDATE SET
                 display_name=excluded.display_name,
                 prefs_json=excluded.prefs_json,
                 traits_json=excluded.traits_json,
                 updated_at=excluded.updated_at"#,
            params![display_name, prefs_json, traits_json, updated_at],
        )?;
        Ok(())
    })?;
    Ok(UserProfileRow {
        profile_id: "local".into(),
        display_name: display_name.map(String::from),
        prefs_json: prefs_json.into(),
        traits_json: traits_json.into(),
        updated_at,
    })
}

/// Get rich memory for a project (default empty row when missing).
pub fn get_project_memory(config: &Config, project: &str) -> Result<ProjectMemoryRow> {
    let project = normalize_project(project);
    with_conn(config, |conn| {
        let row = conn
            .query_row(
                "SELECT project, summary, open_tensions_json, last_role_pack, last_brief_json, signals_json, updated_at
                 FROM project_memory WHERE project = ?1",
                params![project],
                |r| {
                    Ok(ProjectMemoryRow {
                        project: r.get(0)?,
                        summary: r.get(1)?,
                        open_tensions_json: r.get(2)?,
                        last_role_pack: r.get(3)?,
                        last_brief_json: r.get(4)?,
                        signals_json: r.get(5)?,
                        updated_at: r.get(6)?,
                    })
                },
            )
            .optional()?;
        if let Some(row) = row {
            return Ok(row);
        }
        let row = ProjectMemoryRow {
            project: project.clone(),
            summary: String::new(),
            open_tensions_json: "[]".into(),
            last_role_pack: None,
            last_brief_json: None,
            signals_json: "{}".into(),
            updated_at: Utc::now().to_rfc3339(),
        };
        conn.execute(
            r#"INSERT INTO project_memory (project, summary, open_tensions_json, last_role_pack, last_brief_json, signals_json, updated_at)
               VALUES (?1, '', '[]', NULL, NULL, '{}', ?2)"#,
            params![project, row.updated_at],
        )?;
        Ok(row)
    })
}

/// Upsert rich memory for a project (G1+ writes here; reuses thin tables, no dual-write).
pub fn upsert_project_memory(
    config: &Config,
    project: &str,
    summary: &str,
    open_tensions_json: &str,
    last_role_pack: Option<&str>,
    last_brief_json: Option<&str>,
    signals_json: &str,
) -> Result<()> {
    let project = normalize_project(project);
    if project.is_empty() {
        bail!("project empty");
    }
    let updated_at = Utc::now().to_rfc3339();
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO project_memory
                 (project, summary, open_tensions_json, last_role_pack, last_brief_json, signals_json, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(project) DO UPDATE SET
                 summary=excluded.summary,
                 open_tensions_json=excluded.open_tensions_json,
                 last_role_pack=excluded.last_role_pack,
                 last_brief_json=excluded.last_brief_json,
                 signals_json=excluded.signals_json,
                 updated_at=excluded.updated_at"#,
            params![project, summary, open_tensions_json, last_role_pack, last_brief_json, signals_json, updated_at],
        )?;
        Ok(())
    })
}

/// Start a guided session (G0 shell: no slots/brief yet, status `active`).
pub fn start_session(
    config: &Config,
    project: &str,
    mode: SessionMode,
    entry: SessionEntry,
    role_pack: &str,
) -> Result<GuideSession> {
    let project = normalize_project(project);
    if project.is_empty() {
        bail!("project empty");
    }
    if role_pack.trim().is_empty() {
        bail!("role_pack empty");
    }
    let now = Utc::now().to_rfc3339();
    let session = GuideSession {
        session_id: new_session_id(),
        project: project.clone(),
        mode,
        entry,
        status: SessionStatus::Active,
        role_pack: role_pack.trim().to_string(),
        slots: serde_json::json!({}),
        brief: None,
        plan_path: None,
        created_at: now.clone(),
        updated_at: now,
    };
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO guide_sessions
                 (session_id, project, mode, entry, status, role_pack, slots_json, brief_json, plan_path, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                session.session_id,
                session.project,
                session.mode.as_str(),
                session.entry.as_str(),
                session.status.as_str(),
                session.role_pack,
                serde_json::to_string(&session.slots)?,
                session.brief.as_ref().map(|b| serde_json::to_string(b)).transpose()?,
                session.plan_path,
                session.created_at,
                session.updated_at,
            ],
        )?;
        Ok(())
    })?;
    Ok(session)
}

/// Get one session (None when missing).
pub fn get_session(config: &Config, session_id: &str) -> Result<Option<GuideSession>> {
    with_conn(config, |conn| {
        Ok(conn
            .query_row(
                "SELECT session_id, project, mode, entry, status, role_pack, slots_json, brief_json, plan_path, created_at, updated_at
                 FROM guide_sessions WHERE session_id = ?1",
                params![session_id],
                row_to_session,
            )
            .optional()?)
    })
}

/// List sessions for a project (newest first).
pub fn list_sessions(config: &Config, project: &str) -> Result<Vec<GuideSession>> {
    let project = normalize_project(project);
    if project.is_empty() {
        return Ok(vec![]);
    }
    with_conn(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT session_id, project, mode, entry, status, role_pack, slots_json, brief_json, plan_path, created_at, updated_at
             FROM guide_sessions WHERE project = ?1
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![project], row_to_session)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    })
}

fn row_to_session(r: &rusqlite::Row<'_>) -> rusqlite::Result<GuideSession> {
    let mode_raw: String = r.get(2)?;
    let entry_raw: String = r.get(3)?;
    let status_raw: String = r.get(4)?;
    let slots_json: String = r.get(6)?;
    let brief_json: Option<String> = r.get(7)?;
    Ok(GuideSession {
        session_id: r.get(0)?,
        project: r.get(1)?,
        mode: SessionMode::parse(&mode_raw).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                format!("unknown session mode {mode_raw:?}").into(),
            )
        })?,
        entry: SessionEntry::parse(&entry_raw).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("unknown session entry {entry_raw:?}").into(),
            )
        })?,
        status: SessionStatus::parse(&status_raw).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                format!("unknown session status {status_raw:?}").into(),
            )
        })?,
        role_pack: r.get(5)?,
        slots: serde_json::from_str(&slots_json).unwrap_or_else(|_| serde_json::json!({})),
        brief: parse_brief(brief_json),
        plan_path: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn normalize_project(project: &str) -> String {
    project.trim_end_matches('/').to_string()
}

/// Parse `brief_json` column; malformed JSON degrades to None (best-effort read).
fn parse_brief(json: Option<String>) -> Option<GuideBrief> {
    json.as_deref()
        .and_then(|s| serde_json::from_str::<GuideBrief>(s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sqlite::reset_for_test;
    use tempfile::tempdir;

    fn test_cfg() -> (tempfile::TempDir, Config) {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        (dir, cfg)
    }

    #[test]
    fn profile_get_creates_default_then_upserts() {
        let (_d, cfg) = test_cfg();
        let p = get_user_profile(&cfg).unwrap();
        assert_eq!(p.profile_id, "local");
        assert_eq!(p.prefs_json, "{}");
        upsert_user_profile(&cfg, Some("小明"), r#"{"lang":"zh"}"#, r#"{"risk":"low"}"#).unwrap();
        let p2 = get_user_profile(&cfg).unwrap();
        assert_eq!(p2.display_name.as_deref(), Some("小明"));
        assert!(p2.prefs_json.contains("zh"));
    }

    #[test]
    fn project_memory_get_default_then_upsert() {
        let (_d, cfg) = test_cfg();
        let m = get_project_memory(&cfg, "/tmp/guide-proj").unwrap();
        assert_eq!(m.summary, "");
        assert_eq!(m.open_tensions_json, "[]");
        upsert_project_memory(
            &cfg,
            "/tmp/guide-proj/",
            "卡在成本 vs 上线时间",
            r#"[{"a":"cost","b":"time"}]"#,
            Some("ship-product"),
            None,
            r#"{"budget_edits":2}"#,
        )
        .unwrap();
        let m2 = get_project_memory(&cfg, "/tmp/guide-proj").unwrap();
        assert_eq!(m2.summary, "卡在成本 vs 上线时间");
        assert_eq!(m2.last_role_pack.as_deref(), Some("ship-product"));
    }

    #[test]
    fn session_start_get_list_roundtrip() {
        let (_d, cfg) = test_cfg();
        let s = start_session(
            &cfg,
            "/tmp/guide-proj/",
            SessionMode::Coop,
            SessionEntry::Socratic,
            "ship-product",
        )
        .unwrap();
        assert_eq!(s.status, SessionStatus::Active);
        assert!(s.session_id.starts_with("g20"));

        let got = get_session(&cfg, &s.session_id)
            .unwrap()
            .expect("session found");
        assert_eq!(got, s);

        let list = list_sessions(&cfg, "/tmp/guide-proj").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].session_id, s.session_id);
        // Different project is isolated.
        assert!(list_sessions(&cfg, "/tmp/other").unwrap().is_empty());
    }

    #[test]
    fn start_rejects_empty_project_or_pack() {
        let (_d, cfg) = test_cfg();
        assert!(start_session(&cfg, "", SessionMode::Coop, SessionEntry::Quick, "p").is_err());
        assert!(
            start_session(&cfg, "/tmp/x", SessionMode::Coop, SessionEntry::Quick, "  ").is_err()
        );
    }
}
