//! SQLite store for plan jobs + cco split SoT.
//!
//! [INPUT]: Config.state_root · PlanJob · PlanIR · CcoSplitJob
//! [OUTPUT]: ~/.cco/cco.db — plan_jobs/plan_tasks + cco_split_* + project_last_summary/project_pins + project_ui_prefs
//! [POS]: state adapter
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md
//!
//! Product: cco_split_jobs/tasks = split SoT (full fields). plan_tasks dual-write remains
//! for legacy query until consumers migrate.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::config::Config;
use crate::plan::planner::PlanJob;
use crate::plan::PlanIR;

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn db_path(config: &Config) -> PathBuf {
    config.state_root.join("cco.db")
}

/// Create all tables (idempotent). Safe on every open / migration.
pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS plan_jobs (
          job_id TEXT PRIMARY KEY,
          project TEXT NOT NULL,
          plan_path TEXT NOT NULL,
          status TEXT NOT NULL,
          plan_mode TEXT,
          provider TEXT,
          exec_mode TEXT,
          plan_name TEXT,
          task_count INTEGER,
          max_parallel INTEGER,
          adapter TEXT,
          error TEXT,
          run_id TEXT,
          planner_cost_usd REAL,
          digest_mode TEXT,
          critic_summary TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_plan_jobs_project_updated
          ON plan_jobs(project, updated_at DESC);
        CREATE TABLE IF NOT EXISTS plan_tasks (
          job_id TEXT NOT NULL,
          task_id TEXT NOT NULL,
          ord INTEGER NOT NULL,
          title TEXT NOT NULL,
          optional INTEGER NOT NULL DEFAULT 0,
          include INTEGER NOT NULL DEFAULT 1,
          depends_on TEXT NOT NULL DEFAULT '[]',
          wave INTEGER,
          role TEXT,
          provider TEXT,
          group_name TEXT,
          prompt_preview TEXT,
          PRIMARY KEY (job_id, task_id)
        );
        CREATE INDEX IF NOT EXISTS idx_plan_tasks_job ON plan_tasks(job_id, ord);

        -- cco-native split SoT (C2)
        CREATE TABLE IF NOT EXISTS cco_split_jobs (
          job_id TEXT PRIMARY KEY,
          project TEXT NOT NULL,
          plan_path TEXT NOT NULL,
          status TEXT NOT NULL,
          title TEXT NOT NULL,
          max_parallel INTEGER NOT NULL DEFAULT 1,
          source TEXT NOT NULL DEFAULT 'heuristic',
          error TEXT,
          run_id TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_cco_split_jobs_project
          ON cco_split_jobs(project, updated_at DESC);
        CREATE TABLE IF NOT EXISTS cco_split_tasks (
          job_id TEXT NOT NULL,
          task_id TEXT NOT NULL,
          ord INTEGER NOT NULL,
          title TEXT NOT NULL,
          summary TEXT NOT NULL DEFAULT '',
          body TEXT NOT NULL,
          depends_on TEXT NOT NULL DEFAULT '[]',
          wave INTEGER NOT NULL DEFAULT 0,
          enabled INTEGER NOT NULL DEFAULT 1,
          optional INTEGER NOT NULL DEFAULT 0,
          done_when TEXT,
          verify_cmd TEXT,
          plan_ref TEXT,
          kind TEXT NOT NULL DEFAULT 'do',
          status TEXT NOT NULL DEFAULT 'pending',
          provider TEXT,
          role TEXT,
          scope_paths TEXT NOT NULL DEFAULT '[]',
          meta_json TEXT,
          PRIMARY KEY (job_id, task_id)
        );
        CREATE INDEX IF NOT EXISTS idx_cco_split_tasks_job
          ON cco_split_tasks(job_id, ord);

        -- P2-2 project light memory (last_summary + pins ≤3)
        CREATE TABLE IF NOT EXISTS project_last_summary (
          project_id TEXT PRIMARY KEY,
          text TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS project_pins (
          project_id TEXT NOT NULL,
          key TEXT NOT NULL,
          value TEXT NOT NULL,
          pinned_at TEXT NOT NULL,
          PRIMARY KEY (project_id, key)
        );
        CREATE INDEX IF NOT EXISTS idx_project_pins_project
          ON project_pins(project_id, pinned_at DESC);

        -- Per-project UI prefs (dismissed run, etc.) — durable SoT, not memory/localStorage
        CREATE TABLE IF NOT EXISTS project_ui_prefs (
          project_id TEXT NOT NULL,
          key TEXT NOT NULL,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          PRIMARY KEY (project_id, key)
        );
        CREATE INDEX IF NOT EXISTS idx_project_ui_prefs_project
          ON project_ui_prefs(project_id);
        "#,
    )?;
    // H2: additive columns on existing DBs (CREATE IF NOT EXISTS alone is not enough).
    ensure_column(conn, "cco_split_tasks", "verify_cmd", "TEXT")?;
    Ok(())
}

/// `PRAGMA table_info` + `ALTER TABLE … ADD COLUMN` when missing (H2 migration).
pub(crate) fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> Result<()> {
    // table/column/decl are internal constants only — not user input.
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if names.iter().any(|n| n == column) {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
        [],
    )
    .with_context(|| format!("add column {table}.{column}"))?;
    Ok(())
}

pub(crate) fn with_conn<F, T>(config: &Config, f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let mut guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let path = db_path(config);
    let need_open = match guard.as_ref() {
        None => true,
        // Tests / multi-root: reopen when state_root (db path) changes.
        Some(conn) => conn
            .path()
            .map(|p| PathBuf::from(p) != path)
            .unwrap_or(true),
    };
    if need_open {
        std::fs::create_dir_all(&config.state_root)
            .with_context(|| format!("mkdir {}", config.state_root.display()))?;
        let conn = Connection::open(&path)
            .with_context(|| format!("open sqlite {}", path.display()))?;
        ensure_schema(&conn)?;
        *guard = Some(conn);
    }
    let conn = guard.as_ref().expect("db open");
    // Upgrade path: existing process might have opened before cco_split tables existed.
    ensure_schema(conn)?;
    f(conn)
}

/// Upsert plan job row (best-effort; never fails the planner path).
pub fn upsert_plan_job(config: &Config, job: &PlanJob) -> Result<()> {
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO plan_jobs (
                job_id, project, plan_path, status, plan_mode, provider, exec_mode,
                plan_name, task_count, max_parallel, adapter, error, run_id,
                planner_cost_usd, digest_mode, critic_summary, created_at, updated_at
              ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
              ON CONFLICT(job_id) DO UPDATE SET
                project=excluded.project,
                plan_path=excluded.plan_path,
                status=excluded.status,
                plan_mode=excluded.plan_mode,
                provider=excluded.provider,
                exec_mode=excluded.exec_mode,
                plan_name=excluded.plan_name,
                task_count=excluded.task_count,
                max_parallel=excluded.max_parallel,
                adapter=excluded.adapter,
                error=excluded.error,
                run_id=excluded.run_id,
                planner_cost_usd=excluded.planner_cost_usd,
                digest_mode=excluded.digest_mode,
                critic_summary=excluded.critic_summary,
                updated_at=excluded.updated_at
            "#,
            params![
                job.job_id,
                job.project.to_string_lossy(),
                job.plan_path.to_string_lossy(),
                job.status.as_str(),
                job.plan_mode,
                job.provider,
                job.exec_mode,
                job.plan_name,
                job.task_count.map(|n| n as i64),
                job.max_parallel.map(|n| n as i64),
                job.adapter,
                job.error,
                job.run_id,
                job.planner_cost_usd,
                job.digest_mode,
                job.critic_summary,
                job.created_at.to_rfc3339(),
                job.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })
}

/// Replace proposed tasks for a job (display fields for UI / query).
pub fn replace_plan_tasks(config: &Config, job_id: &str, ir: &PlanIR) -> Result<()> {
    use crate::graph::topo_layers;
    let layers = topo_layers(ir);
    let mut wave_of: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (wi, layer) in layers.iter().enumerate() {
        for id in layer {
            wave_of.insert(id.as_str(), wi);
        }
    }

    with_conn(config, |conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM plan_tasks WHERE job_id = ?1", params![job_id])?;
        {
            let mut stmt = tx.prepare(
                r#"INSERT INTO plan_tasks (
                    job_id, task_id, ord, title, optional, include, depends_on,
                    wave, role, provider, group_name, prompt_preview
                  ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"#,
            )?;
            for (ord, t) in ir.tasks.iter().enumerate() {
                let deps = serde_json::to_string(&t.depends_on).unwrap_or_else(|_| "[]".into());
                let preview: String = t.prompt.chars().take(240).collect();
                let role = t.role.map(|r| r.as_str().to_string());
                let wave = wave_of.get(t.id.as_str()).map(|w| *w as i64);
                stmt.execute(params![
                    job_id,
                    t.id,
                    ord as i64,
                    t.title,
                    t.optional as i64,
                    t.include as i64,
                    deps,
                    wave,
                    role,
                    t.provider,
                    t.group,
                    preview,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

/// Best-effort dual-write helpers (log errors, never poison planner).
pub fn try_upsert_plan_job(config: &Config, job: &PlanJob) {
    if let Err(e) = upsert_plan_job(config, job) {
        tracing::warn!(error = %e, job_id = %job.job_id, "sqlite upsert plan_job failed");
    }
}

pub fn try_replace_plan_tasks(config: &Config, job_id: &str, ir: &PlanIR) {
    if let Err(e) = replace_plan_tasks(config, job_id, ir) {
        tracing::warn!(error = %e, job_id = %job_id, "sqlite replace plan_tasks failed");
    }
}

/// Lightweight row: plan list / rail「已拆分」索引（读 `plan_jobs` dual-write）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSplitIndexRow {
    pub job_id: String,
    pub plan_path: String,
    pub status: String,
    pub task_count: Option<i64>,
    pub plan_name: Option<String>,
    pub updated_at: String,
    /// parse | fake | ai | direct | fast …
    #[serde(default)]
    pub plan_mode: Option<String>,
    /// raw-single / planner-ai-llm / cco-split/… — used for restore quality rank.
    #[serde(default)]
    pub adapter: Option<String>,
}

/// Desk restore / prior-success rank for **status** (higher wins among same quality).
///
/// Note: quality (multi-step AI vs direct 1-step) is ranked separately via
/// [`split_graph_quality`] — a planned 8-step AI graph must beat a confirmed
/// direct 1-step when recovering after a failed re-split.
pub fn split_status_rank(status: &str) -> u8 {
    match status.trim().to_ascii_lowercase().as_str() {
        "confirmed" => 3,
        "planned" | "ready" => 2,
        "planning" => 1,
        _ => 0,
    }
}

/// Graph quality for restore preference (higher = better desk to keep).
///
/// Rules (product, 2026-07-24):
/// 1. **direct / raw-single** (整份 1 步) = lowest non-zero quality — never hide a
///    multi-step AI split just because direct was confirmed first.
/// 2. Multi-step graphs (`task_count ≥ 2`) outrank single-step.
/// 3. LLM / cco-split adapters outrank heuristic / parse residuals of same size.
/// 4. Larger task_count wins ties (capped so one huge residual cannot dominate forever).
pub fn split_graph_quality(
    plan_mode: Option<&str>,
    adapter: Option<&str>,
    task_count: Option<u32>,
) -> u32 {
    let mode = plan_mode.unwrap_or("").trim().to_ascii_lowercase();
    let adapter = adapter.unwrap_or("").trim().to_ascii_lowercase();
    let n = task_count.unwrap_or(0).min(64);

    let is_direct = mode == "direct"
        || adapter == "raw-single"
        || adapter.ends_with("/raw-single")
        || (n <= 1 && (adapter.contains("raw-single") || mode == "direct"));

    if is_direct {
        // Confirmed direct still restorable, but loses to any multi-step graph.
        return 10 + n.min(1);
    }

    let mut q: u32 = 100;
    // Prefer real multi-step graphs.
    if n >= 2 {
        q += 200 + n.saturating_mul(3);
    } else if n == 1 {
        q += 20;
    }

    let is_llm = adapter.contains("llm")
        || adapter.contains("cco-split")
        || adapter.contains("split-agent")
        || mode == "ai";
    let is_heuristic = adapter.contains("heuristic") || mode == "fast" || mode == "heuristic";
    if is_llm {
        q += 80;
    } else if is_heuristic {
        q += 30;
    } else if mode == "parse" || adapter.contains("serial") || adapter.contains("cco-v1") {
        q += 40;
    }

    q
}

/// Compare two split candidates for desk restore / prior success.
/// Order: quality desc → status rank desc → updated_at desc → job_id desc.
pub fn cmp_split_restore(
    a_quality: u32,
    a_status: &str,
    a_updated: &str,
    a_job_id: &str,
    b_quality: u32,
    b_status: &str,
    b_updated: &str,
    b_job_id: &str,
) -> std::cmp::Ordering {
    a_quality
        .cmp(&b_quality)
        .then_with(|| split_status_rank(a_status).cmp(&split_status_rank(b_status)))
        .then_with(|| a_updated.cmp(b_updated))
        .then_with(|| a_job_id.cmp(b_job_id))
}

/// Normalize plan path for equality (relative preferred; slash-unified).
pub fn plan_path_key(plan_path: &str) -> String {
    let mut s = plan_path.trim().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("file://") {
        s = rest.to_string();
    }
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    s.trim_start_matches('/').to_string()
}

pub fn plan_paths_match(a: &str, b: &str) -> bool {
    let ka = plan_path_key(a);
    let kb = plan_path_key(b);
    if ka.is_empty() || kb.is_empty() {
        return false;
    }
    ka == kb || ka.ends_with(&kb) || kb.ends_with(&ka)
}

fn project_key(project: &Path) -> String {
    project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf())
        .to_string_lossy()
        .to_string()
}

/// Latest recoverable job_id for a plan path (planned / confirmed / planning).
///
/// Source: SQLite `plan_jobs` (dual-write on every job.save). Used by plan list
/// 「查看拆分结果」so memory `state.planJob` is not the only gate.
///
/// Preference (2026-07-24):
/// 1. **Graph quality** — multi-step AI/cco-split ≫ direct/raw-single 1-step
/// 2. Status rank among same quality (confirmed > planned > planning)
/// 3. Newer `updated_at`
///
/// Incomplete re-splits must not hide a better prior graph; a confirmed direct
/// 1-step must **not** hide a planned 8-step AI desk.
pub fn latest_job_id_for_plan_path(
    config: &Config,
    project: &Path,
    plan_path: &str,
) -> Result<Option<String>> {
    let rows = list_plan_split_index(config, project)?;
    let want = plan_path_key(plan_path);
    if want.is_empty() {
        return Ok(None);
    }
    let mut matched: Vec<PlanSplitIndexRow> = rows
        .into_iter()
        .filter(|r| plan_paths_match(&r.plan_path, plan_path))
        .collect();
    if matched.is_empty() {
        return Ok(None);
    }
    matched.sort_by(|a, b| {
        let qa = split_graph_quality(
            a.plan_mode.as_deref(),
            a.adapter.as_deref(),
            a.task_count.map(|n| n.max(0) as u32),
        );
        let qb = split_graph_quality(
            b.plan_mode.as_deref(),
            b.adapter.as_deref(),
            b.task_count.map(|n| n.max(0) as u32),
        );
        // sort_by wants ascending comparator; reverse for "best first"
        cmp_split_restore(
            qb,
            &b.status,
            &b.updated_at,
            &b.job_id,
            qa,
            &a.status,
            &a.updated_at,
            &a.job_id,
        )
    });
    Ok(matched.into_iter().next().map(|r| r.job_id))
}

/// All recoverable split index rows for a project (newest first).
/// Status filter: planning | planned | confirmed (desk-restorable).
pub fn list_plan_split_index(
    config: &Config,
    project: &Path,
) -> Result<Vec<PlanSplitIndexRow>> {
    let proj = project_key(project);
    let proj_raw = project.to_string_lossy().to_string();
    with_conn(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            r#"SELECT job_id, plan_path, status, task_count, plan_name, updated_at, project,
                      plan_mode, adapter
               FROM plan_jobs
               WHERE status IN ('planning','planned','confirmed')
               ORDER BY updated_at DESC"#,
        )?;
        let mut out = Vec::new();
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?;
        for row in rows {
            let (
                job_id,
                plan_path,
                status,
                task_count,
                plan_name,
                updated_at,
                row_project,
                plan_mode,
                adapter,
            ) = row?;
            // project column may be absolute or not-canonicalized
            let rp = PathBuf::from(&row_project);
            let same = project_key(&rp) == proj
                || row_project == proj_raw
                || row_project == proj
                || proj.ends_with(&row_project)
                || row_project.ends_with(&proj_raw);
            if !same {
                continue;
            }
            out.push(PlanSplitIndexRow {
                job_id,
                plan_path,
                status,
                task_count,
                plan_name,
                updated_at,
                plan_mode,
                adapter,
            });
        }
        Ok(out)
    })
}

/// Path of the DB file (tests / doctor).
pub fn sqlite_path(config: &Config) -> PathBuf {
    db_path(config)
}

/// Reset in-process connection (tests with temp state_root).
#[cfg(test)]
pub fn reset_for_test() {
    let mut g = DB.lock().unwrap_or_else(|e| e.into_inner());
    *g = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn graph_quality_direct_loses_to_multi_step_ai() {
        let q_direct = split_graph_quality(Some("direct"), Some("raw-single"), Some(1));
        let q_ai8 = split_graph_quality(Some("ai"), Some("planner-ai-llm"), Some(8));
        let q_ai1 = split_graph_quality(Some("ai"), Some("planner-ai-llm"), Some(1));
        assert!(
            q_ai8 > q_direct,
            "8-step AI ({q_ai8}) must beat direct 1 ({q_direct})"
        );
        assert!(
            q_ai8 > q_ai1,
            "8-step AI must beat 1-step AI residual"
        );
        // Confirmed vs planned is status rank, not quality — both multi-step AI close.
        let q_confirmed7 =
            split_graph_quality(Some("ai"), Some("planner-ai-llm"), Some(7));
        let q_heur5 =
            split_graph_quality(Some("ai"), Some("planner-ai-heuristic"), Some(5));
        assert!(
            q_confirmed7 > q_heur5,
            "larger LLM graph outranks smaller heuristic"
        );
    }
    use crate::plan::planner::{PlanJob, PlanJobStatus};
    use crate::plan::{OnFailure, PlanIR, TaskIR};
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn dual_write_job_and_tasks() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        let job = PlanJob {
            job_id: "plan-test-1".into(),
            status: PlanJobStatus::Planned,
            project: PathBuf::from("/tmp/p"),
            plan_path: PathBuf::from("docs/x.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            plan_name: Some("x".into()),
            task_count: Some(2),
            max_parallel: Some(2),
            adapter: Some("test".into()),
            planner_cost_usd: None,
            digest_mode: Some("greenfield".into()),
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        upsert_plan_job(&cfg, &job).unwrap();
        let ir = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "x".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("docs/x.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                TaskIR {
                    id: "t1".into(),
                    title: "A1 · first".into(),
                    depends_on: vec![],
                    group: Some("G1".into()),
                    provider: "claude".into(),
                    mode: "print".into(),
                    prompt: "do one".into(),
                    verify_cmd: None,
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: false,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
                TaskIR {
                    id: "t2".into(),
                    title: "A2 · second".into(),
                    depends_on: vec!["t1".into()],
                    group: Some("G2".into()),
                    provider: "claude".into(),
                    mode: "print".into(),
                    prompt: "do two".into(),
                    verify_cmd: None,
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: true,
                    include: false,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
            ],
        };
        replace_plan_tasks(&cfg, "plan-test-1", &ir).unwrap();
        with_conn(&cfg, |conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM plan_tasks WHERE job_id='plan-test-1'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(n, 2);
            let inc: i64 = conn.query_row(
                "SELECT include FROM plan_tasks WHERE task_id='t2'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(inc, 0);
            Ok(())
        })
        .unwrap();
        assert!(sqlite_path(&cfg).is_file());
    }

    #[test]
    fn latest_job_id_for_plan_path_from_index() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        let project = PathBuf::from("/tmp/proj-split-idx");
        let older = PlanJob {
            job_id: "plan-old".into(),
            status: PlanJobStatus::Planned,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now() - chrono::Duration::seconds(60),
            plan_name: Some("old".into()),
            task_count: Some(1),
            max_parallel: Some(1),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        let newer = PlanJob {
            job_id: "plan-new".into(),
            status: PlanJobStatus::Planned,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            plan_name: Some("new".into()),
            task_count: Some(3),
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        let other = PlanJob {
            job_id: "plan-other".into(),
            status: PlanJobStatus::Planned,
            project: project.clone(),
            plan_path: PathBuf::from("docs/b.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            plan_name: Some("b".into()),
            task_count: Some(2),
            max_parallel: Some(1),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        upsert_plan_job(&cfg, &older).unwrap();
        upsert_plan_job(&cfg, &newer).unwrap();
        upsert_plan_job(&cfg, &other).unwrap();

        let id = latest_job_id_for_plan_path(&cfg, &project, "docs/a.md")
            .unwrap()
            .expect("job id");
        assert_eq!(id, "plan-new");

        // Same-quality multi-step: confirmed AI must beat a newer smaller residual.
        let confirmed = PlanJob {
            job_id: "plan-confirmed".into(),
            status: PlanJobStatus::Confirmed,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: Some("run-1".into()),
            created_at: Utc::now() - chrono::Duration::seconds(120),
            updated_at: Utc::now() - chrono::Duration::seconds(90),
            plan_name: Some("confirmed".into()),
            task_count: Some(7),
            max_parallel: Some(2),
            adapter: Some("planner-ai-llm".into()),
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        let residual = PlanJob {
            job_id: "plan-residual".into(),
            status: PlanJobStatus::Planned,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            plan_name: Some("residual-5".into()),
            task_count: Some(5),
            max_parallel: Some(2),
            adapter: Some("planner-ai-heuristic".into()),
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        upsert_plan_job(&cfg, &confirmed).unwrap();
        upsert_plan_job(&cfg, &residual).unwrap();
        let prefer = latest_job_id_for_plan_path(&cfg, &project, "docs/a.md")
            .unwrap()
            .expect("prefer confirmed multi-step");
        assert_eq!(
            prefer, "plan-confirmed",
            "confirmed multi-step AI must beat smaller heuristic residual"
        );

        // Product bug 2026-07-24: confirmed direct 1-step must NOT hide planned 8-step AI.
        let direct1 = PlanJob {
            job_id: "plan-direct-1".into(),
            status: PlanJobStatus::Confirmed,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "direct".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: Some("run-d".into()),
            created_at: Utc::now() - chrono::Duration::seconds(200),
            updated_at: Utc::now() - chrono::Duration::seconds(10),
            plan_name: Some("direct".into()),
            task_count: Some(1),
            max_parallel: Some(1),
            adapter: Some("raw-single".into()),
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        let ai8 = PlanJob {
            job_id: "plan-ai-8".into(),
            status: PlanJobStatus::Planned,
            project: project.clone(),
            plan_path: PathBuf::from("docs/a.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now() - chrono::Duration::seconds(180),
            updated_at: Utc::now() - chrono::Duration::seconds(60),
            plan_name: Some("ai-8".into()),
            task_count: Some(8),
            max_parallel: Some(2),
            adapter: Some("planner-ai-llm".into()),
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
        };
        upsert_plan_job(&cfg, &direct1).unwrap();
        upsert_plan_job(&cfg, &ai8).unwrap();
        let prefer_ai = latest_job_id_for_plan_path(&cfg, &project, "docs/a.md")
            .unwrap()
            .expect("prefer multi-step AI over direct");
        assert_eq!(
            prefer_ai, "plan-ai-8",
            "planned 8-step AI must beat confirmed direct 1-step"
        );

        let idx = list_plan_split_index(&cfg, &project).unwrap();
        assert!(idx.iter().any(|r| r.plan_path.contains("a.md")));
        assert!(idx.iter().any(|r| r.plan_path.contains("b.md")));
        assert_eq!(
            latest_job_id_for_plan_path(&cfg, &project, "docs/missing.md").unwrap(),
            None
        );
    }
}
