//! SQLite SoT for cco-native split docs (`cco_split_jobs` / `cco_split_tasks`).
//!
//! [INPUT]: Config · CcoSplitJob
//! [OUTPUT]: persist / load full split fields (not PlanIR mirror only)
//! [POS]: state adapter — **拆分 SoT**；plan.proposed.json 为迁移快照
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md · docs/cco-split-format-sqlite-2026-07-21.md

use anyhow::Result;
use rusqlite::{params, OptionalExtension};

use crate::config::Config;
use crate::domain::plan::{
    CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask, CcoTaskKind, CcoTaskStatus,
};

use super::sqlite::{ensure_schema, with_conn};

/// Upsert full split job + replace all tasks (transactional).
pub fn save_cco_split(config: &Config, doc: &CcoSplitJob) -> Result<()> {
    with_conn(config, |conn| {
        ensure_schema(conn)?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"INSERT INTO cco_split_jobs (
                job_id, project, plan_path, status, title, max_parallel, source,
                error, run_id, created_at, updated_at
              ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
              ON CONFLICT(job_id) DO UPDATE SET
                project=excluded.project,
                plan_path=excluded.plan_path,
                status=excluded.status,
                title=excluded.title,
                max_parallel=excluded.max_parallel,
                source=excluded.source,
                error=excluded.error,
                run_id=excluded.run_id,
                updated_at=excluded.updated_at
            "#,
            params![
                doc.job_id,
                doc.project.to_string_lossy(),
                doc.plan_path.to_string_lossy(),
                doc.status.as_str(),
                doc.title,
                doc.max_parallel as i64,
                doc.source.as_str(),
                doc.error,
                doc.run_id,
                doc.created_at,
                doc.updated_at,
            ],
        )?;
        tx.execute(
            "DELETE FROM cco_split_tasks WHERE job_id = ?1",
            params![doc.job_id],
        )?;
        {
            let mut stmt = tx.prepare(
                r#"INSERT INTO cco_split_tasks (
                    job_id, task_id, ord, title, summary, body, depends_on, wave,
                    enabled, optional, done_when, plan_ref, kind, status,
                    provider, role, scope_paths, meta_json
                  ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)"#,
            )?;
            for t in &doc.tasks {
                let deps = serde_json::to_string(&t.depends_on).unwrap_or_else(|_| "[]".into());
                let scope =
                    serde_json::to_string(&t.scope_paths).unwrap_or_else(|_| "[]".into());
                let meta = t
                    .meta_json
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));
                stmt.execute(params![
                    doc.job_id,
                    t.task_id,
                    t.ord as i64,
                    t.title,
                    t.summary,
                    t.body,
                    deps,
                    t.wave as i64,
                    t.enabled as i64,
                    t.optional as i64,
                    t.done_when,
                    t.plan_ref,
                    t.kind.as_str(),
                    t.status.as_str(),
                    t.provider,
                    t.role,
                    scope,
                    meta,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

/// Load split job from SQLite SoT. `None` if no row.
pub fn load_cco_split(config: &Config, job_id: &str) -> Result<Option<CcoSplitJob>> {
    with_conn(config, |conn| {
        ensure_schema(conn)?;
        let row = conn
            .query_row(
                r#"SELECT job_id, project, plan_path, status, title, max_parallel, source,
                          error, run_id, created_at, updated_at
                   FROM cco_split_jobs WHERE job_id = ?1"#,
                params![job_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, String>(9)?,
                        r.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            job_id,
            project,
            plan_path,
            status,
            title,
            max_parallel,
            source,
            error,
            run_id,
            created_at,
            updated_at,
        )) = row
        else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            r#"SELECT task_id, ord, title, summary, body, depends_on, wave,
                      enabled, optional, done_when, plan_ref, kind, status,
                      provider, role, scope_paths, meta_json
               FROM cco_split_tasks WHERE job_id = ?1 ORDER BY ord ASC"#,
        )?;
        let tasks = stmt
            .query_map(params![job_id], |r| {
                let deps_s: String = r.get(5)?;
                let scope_s: String = r.get(15)?;
                let meta_s: Option<String> = r.get(16)?;
                let depends_on: Vec<String> =
                    serde_json::from_str(&deps_s).unwrap_or_default();
                let scope_paths: Vec<String> =
                    serde_json::from_str(&scope_s).unwrap_or_default();
                let meta_json = meta_s
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok());
                Ok(CcoSplitTask {
                    task_id: r.get(0)?,
                    ord: r.get::<_, i64>(1)? as i32,
                    title: r.get(2)?,
                    summary: r.get(3)?,
                    body: r.get(4)?,
                    depends_on,
                    wave: r.get::<_, i64>(6)? as i32,
                    enabled: r.get::<_, i64>(7)? != 0,
                    optional: r.get::<_, i64>(8)? != 0,
                    done_when: r.get(9)?,
                    plan_ref: r.get(10)?,
                    kind: CcoTaskKind::parse(&r.get::<_, String>(11)?),
                    status: CcoTaskStatus::parse(&r.get::<_, String>(12)?),
                    provider: r.get(13)?,
                    role: r.get(14)?,
                    scope_paths,
                    meta_json,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(CcoSplitJob {
            job_id,
            project: std::path::PathBuf::from(project),
            plan_path: std::path::PathBuf::from(plan_path),
            status: CcoSplitStatus::parse(&status),
            title,
            max_parallel: max_parallel.max(1) as usize,
            source: CcoSplitSource::parse(&source),
            error,
            run_id,
            created_at,
            updated_at,
            tasks,
        }))
    })
}

/// Best-effort SoT write (never poison planner).
pub fn try_save_cco_split(config: &Config, doc: &CcoSplitJob) {
    if let Err(e) = save_cco_split(config, doc) {
        tracing::warn!(
            error = %e,
            job_id = %doc.job_id,
            "sqlite save cco_split failed"
        );
    }
}

/// Mark confirmed + run_id on SoT row (best-effort).
pub fn try_mark_cco_split_confirmed(config: &Config, job_id: &str, run_id: &str, updated_at: &str) {
    let res = with_conn(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            r#"UPDATE cco_split_jobs
               SET status = 'confirmed', run_id = ?2, updated_at = ?3
               WHERE job_id = ?1"#,
            params![job_id, run_id, updated_at],
        )?;
        Ok(())
    });
    if let Err(e) = res {
        tracing::warn!(error = %e, job_id = %job_id, "sqlite mark cco_split confirmed failed");
    }
}

/// Patch one task's enabled flag (confirm checkbox) without full rewrite.
pub fn try_set_task_enabled(config: &Config, job_id: &str, task_id: &str, enabled: bool) {
    let res = with_conn(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "UPDATE cco_split_tasks SET enabled = ?3 WHERE job_id = ?1 AND task_id = ?2",
            params![job_id, task_id, enabled as i64],
        )?;
        Ok(())
    });
    if let Err(e) = res {
        tracing::warn!(error = %e, job_id = %job_id, "sqlite set task enabled failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{soft_accept_split, CcoSplitSource, CcoSplitStatus, CcoTaskKind};
    use crate::state::sqlite::reset_for_test;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_roundtrip() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();

        let mut doc = CcoSplitJob {
            job_id: "plan-cco-1".into(),
            project: std::path::PathBuf::from("/tmp/p"),
            plan_path: std::path::PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "demo".into(),
            max_parallel: 2,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "2026-07-21T00:00:00Z".into(),
            updated_at: "2026-07-21T00:00:00Z".into(),
            tasks: vec![
                CcoSplitTask {
                    task_id: "t1".into(),
                    ord: 0,
                    title: "A".into(),
                    summary: "do a".into(),
                    body: "full body a".into(),
                    depends_on: vec![],
                    wave: 0,
                    enabled: true,
                    optional: false,
                    done_when: Some("ok".into()),
                    plan_ref: Some("§1".into()),
                    kind: CcoTaskKind::Do,
                    status: CcoTaskStatus::Pending,
                    provider: Some("claude".into()),
                    role: Some("implement".into()),
                    scope_paths: vec!["src/".into()],
                    meta_json: Some(serde_json::json!({"group": "G1"})),
                },
                CcoSplitTask {
                    task_id: "t2".into(),
                    ord: 1,
                    title: "B".into(),
                    summary: "do b".into(),
                    body: "full body b".into(),
                    depends_on: vec!["t1".into()],
                    wave: 1,
                    enabled: false,
                    optional: true,
                    done_when: None,
                    plan_ref: None,
                    kind: CcoTaskKind::Do,
                    status: CcoTaskStatus::Pending,
                    provider: None,
                    role: None,
                    scope_paths: vec![],
                    meta_json: None,
                },
            ],
        };
        soft_accept_split(&mut doc);
        save_cco_split(&cfg, &doc).unwrap();

        let loaded = load_cco_split(&cfg, "plan-cco-1").unwrap().expect("row");
        assert_eq!(loaded.title, "demo");
        assert_eq!(loaded.tasks.len(), 2);
        assert_eq!(loaded.tasks[0].body, "full body a");
        assert_eq!(loaded.tasks[0].done_when.as_deref(), Some("ok"));
        assert_eq!(loaded.tasks[0].scope_paths, vec!["src/".to_string()]);
        assert!(!loaded.tasks[1].enabled);
        assert!(loaded.tasks[1].optional);
        assert_eq!(loaded.tasks[1].depends_on, vec!["t1".to_string()]);
        assert_eq!(loaded.source, CcoSplitSource::Llm);
    }
}
