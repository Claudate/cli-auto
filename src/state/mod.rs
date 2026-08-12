//! Run state on disk: run.json, events.jsonl, per-task files · SQLite dual-write.
//!
//! [INPUT]: runs_root · PlanIR（初始化）
//! [OUTPUT]: RunState/TaskState(attempt/failover_used/route_*/auto_commit) · AutoCommitPolicySnapshot · save/load · event append · sqlite
//! [POS]: 运行状态落盘；scheduler 与 services 读写
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/state/CLAUDE.md

pub mod cco_split_store;
pub mod guide_store;
pub mod memory_store;
pub mod persona_store;
pub mod project_memory;
pub mod project_ui;
pub mod sqlite;

pub use persona_store::{
    get_project_persona, set_project_persona, try_get_project_persona, try_set_project_persona,
    ProjectPersona,
};
pub use project_memory::{
    compose_last_summary, delete_pin, format_memory_context, get_last_summary, get_memory,
    list_pins, set_last_summary, try_format_memory_context, try_set_last_summary, upsert_pin,
    ProjectLastSummary, ProjectMemoryView, ProjectPin, MAX_PINS_PER_PROJECT,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{AutoCommitGranularity, Config, GitConfig};
use crate::plan::PlanIR;
use crate::runtime::provider::TaskStatus;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Init,
    Validated,
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
}

/// How the current task `provider` was chosen (run.json optional; P1-1).
///
/// Wire values are snake_case. Missing on old runs → `None` (not an error).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteSource {
    /// Plan/confirm kept an explicit provider (not soft-filled).
    Explicit,
    /// Soft-fill applied the plan/default provider.
    SoftFill,
    /// Tag routing rewrote provider after soft-fill (last write wins).
    TagRouting,
    /// Force-provider / hard override.
    Force,
    /// H4 production failover switched provider mid-run.
    Failover,
    /// P0 cost-aware tier pick on still-default tasks.
    CostAuto,
    /// P1 failure cascade: switched to a higher-cost tier.
    CostEscalate,
    /// P2: mid-run budget threshold forced a cheaper tier.
    CostBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAutoCommitResult {
    pub granularity: String,
    pub ok: bool,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
}

/// Git auto-commit policy captured when a run is materialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCommitPolicySnapshot {
    pub granularity: AutoCommitGranularity,
    pub git: GitConfig,
}

impl AutoCommitPolicySnapshot {
    pub fn from_config(config: &Config) -> Self {
        Self {
            granularity: config.auto_commit_granularity(),
            git: config.git.clone(),
        }
    }

    pub fn path(run_dir: &Path) -> PathBuf {
        run_dir.join("auto_commit.json")
    }

    pub fn save(&self, run_dir: &Path) -> Result<()> {
        std::fs::write(Self::path(run_dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load(run_dir: &Path) -> Result<Self> {
        let path = Self::path(run_dir);
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub status: TaskStatus,
    pub provider: String,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminals: Vec<String>,
    /// How many times this task has been started (1 = first try). Used for auto-retry.
    #[serde(default)]
    pub attempt: u32,
    /// Last stall/fail reason short code for UI (stall / fail / timeout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_retry_reason: Option<String>,
    /// True once H4 provider failover has been applied for this task (run-state only).
    #[serde(default)]
    pub failover_used: bool,
    /// Providers already left via H4 failover (for multi-hop order). Old runs omit → [].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failover_tried: Vec<String>,
    /// Provenance of current `provider` (optional; old runs omit → None).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_source: Option<RouteSource>,
    /// Provider name before failover (`route_source=failover`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_previous: Option<String>,
    /// Optional short note (e.g. fail reason code); UI may prefer a composed label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_note: Option<String>,
    /// Host-owned Git auto-commit result for this task (per-task granularity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_commit: Option<TaskAutoCommitResult>,
}

impl TaskState {
    /// Initialize a task before routing provenance is known.
    /// Does **not** hard-code `route_source` (stays `None` until confirm/tag/failover write it).
    pub fn pending(provider: &str, mode: &str) -> Self {
        Self {
            status: TaskStatus::Pending,
            provider: provider.into(),
            mode: mode.into(),
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
            attempt: 0,
            last_retry_reason: None,
            failover_used: false,
            failover_tried: vec![],
            route_source: None,
            route_previous: None,
            route_note: None,
            auto_commit: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub schema: String,
    pub run_id: String,
    pub project_root: PathBuf,
    pub plan_path: PathBuf,
    pub adapter: String,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub tasks: HashMap<String, TaskState>,
    /// Host-owned Git auto-commit results for plan-level commits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_commits: Vec<TaskAutoCommitResult>,
    /// Absolute path to this run directory
    #[serde(skip)]
    pub run_dir: PathBuf,
}

impl RunState {
    pub fn new(run_id: String, project_root: PathBuf, plan: &PlanIR, run_dir: PathBuf) -> Self {
        let mut tasks = HashMap::new();
        for t in &plan.tasks {
            tasks.insert(t.id.clone(), TaskState::pending(&t.provider, &t.mode));
        }
        Self {
            schema: "cco-run/v1".into(),
            run_id,
            project_root,
            plan_path: plan.source_path.clone(),
            adapter: plan.adapter.clone(),
            started_at: Utc::now(),
            finished_at: None,
            status: RunStatus::Init,
            tasks,
            auto_commits: vec![],
            run_dir,
        }
    }

    pub fn run_json_path(&self) -> PathBuf {
        self.run_dir.join("run.json")
    }

    pub fn events_path(&self) -> PathBuf {
        self.run_dir.join("events.jsonl")
    }

    pub fn task_dir(&self, task_id: &str) -> PathBuf {
        self.run_dir.join("tasks").join(task_id)
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.run_dir)?;
        let path = self.run_json_path();
        let mut serializable = self.clone();
        // ensure run_dir not required for serde
        let text = serde_json::to_string_pretty(&serializable)?;
        // re-add run_dir field manually? We skip it — fine.
        let _ = &mut serializable;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn load(run_dir: &Path) -> Result<Self> {
        let path = run_dir.join("run.json");
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut s: RunState = serde_json::from_str(&text)?;
        s.run_dir = run_dir.to_path_buf();
        Ok(s)
    }

    pub fn append_event(&self, event: &serde_json::Value) -> Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(&self.run_dir)?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path())?;
        writeln!(f, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }

    pub fn event(&self, type_name: &str, extra: serde_json::Value) -> Result<()> {
        let mut map = serde_json::Map::new();
        map.insert("ts".into(), serde_json::json!(Utc::now().to_rfc3339()));
        map.insert("type".into(), serde_json::json!(type_name));
        if let serde_json::Value::Object(o) = extra {
            for (k, v) in o {
                map.insert(k, v);
            }
        }
        self.append_event(&serde_json::Value::Object(map))
    }
}

pub fn new_run_id() -> String {
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
    let short = &uuid::Uuid::new_v4().to_string()[..4];
    format!("{ts}-{short}")
}

/// Guide session id (same shape as run id; G0-2).
pub fn new_session_id() -> String {
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
    let short = &uuid::Uuid::new_v4().to_string()[..4];
    format!("g{ts}-{short}")
}

pub fn prepare_run_dir(runs_root: &Path, run_id: &str) -> Result<PathBuf> {
    let dir = runs_root.join(run_id);
    std::fs::create_dir_all(dir.join("tasks"))?;
    Ok(dir)
}

pub fn find_latest_run(runs_root: &Path) -> Result<Option<PathBuf>> {
    if !runs_root.is_dir() {
        return Ok(None);
    }
    let mut dirs: Vec<_> = std::fs::read_dir(runs_root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("run.json").exists())
        .collect();
    dirs.sort();
    Ok(dirs.pop())
}

pub fn resolve_run_dir(runs_root: &Path, run_id: Option<&str>) -> Result<PathBuf> {
    match run_id {
        Some(id) => {
            let d = if Path::new(id).is_absolute() || id.contains('/') {
                PathBuf::from(id)
            } else {
                runs_root.join(id)
            };
            if !d.join("run.json").exists() {
                anyhow::bail!("run not found: {}", d.display());
            }
            Ok(d)
        }
        None => find_latest_run(runs_root)?
            .ok_or_else(|| anyhow::anyhow!("no runs under {}", runs_root.display())),
    }
}

impl RunState {
    /// Reset non-success tasks so scheduler can continue from this run dir.
    /// Manual resume clears attempt counters so the user gets a fresh retry budget.
    pub fn prepare_for_resume(&mut self) -> usize {
        let mut n = 0;
        for ts in self.tasks.values_mut() {
            if matches!(ts.status, TaskStatus::Done | TaskStatus::Skipped) {
                continue;
            }
            ts.status = TaskStatus::Pending;
            ts.error = None;
            ts.finished_at = None;
            ts.started_at = None;
            ts.pid = None;
            ts.attempt = 0;
            ts.last_retry_reason = None;
            ts.failover_used = false;
            n += 1;
        }
        self.status = RunStatus::Init;
        self.finished_at = None;
        n
    }

    /// Reset **one** terminal non-success task so the user can re-run just that step.
    ///
    /// Leaves Done / Skipped / other tasks untouched. Refuses live tasks
    /// (Pending/Queued/Starting/Running) — stop first. Clears attempt counters
    /// for a fresh retry budget on this task only.
    pub fn prepare_task_retry(&mut self, task_id: &str) -> Result<()> {
        let ts = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("任务不存在: {task_id}"))?;
        match ts.status {
            TaskStatus::Done | TaskStatus::Skipped => {
                anyhow::bail!("任务 {task_id} 已完成，无需再跑")
            }
            TaskStatus::Pending
            | TaskStatus::Queued
            | TaskStatus::Starting
            | TaskStatus::Running => {
                anyhow::bail!("任务 {task_id} 仍在进行中，请先停止再重跑")
            }
            TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::Timeout => {
                ts.status = TaskStatus::Pending;
                ts.error = None;
                ts.finished_at = None;
                ts.started_at = None;
                ts.pid = None;
                ts.attempt = 0;
                ts.last_retry_reason = Some("manual".into());
                ts.failover_used = false;
            }
        }
        self.status = RunStatus::Init;
        self.finished_at = None;
        Ok(())
    }

    pub fn total_cost_usd(&self) -> f64 {
        self.tasks.values().filter_map(|t| t.cost_usd).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::TaskStatus;
    use std::io::Write;

    /// P1-1: old run.json TaskState without route_* fields still deserializes.
    #[test]
    fn task_state_deserializes_without_route_fields() {
        let json = r#"{
            "status": "pending",
            "provider": "claude",
            "mode": "print",
            "attempt": 0,
            "failover_used": false
        }"#;
        let ts: TaskState = serde_json::from_str(json).expect("legacy TaskState");
        assert_eq!(ts.status, TaskStatus::Pending);
        assert_eq!(ts.provider, "claude");
        assert_eq!(ts.mode, "print");
        assert!(ts.route_source.is_none());
        assert!(ts.route_previous.is_none());
        assert!(ts.route_note.is_none());
        assert!(!ts.failover_used);
    }

    /// P1-1: full route provenance round-trips through serde (snake_case wire).
    #[test]
    fn task_state_route_fields_round_trip() {
        let mut ts = TaskState::pending("codex", "print");
        ts.route_source = Some(RouteSource::Failover);
        ts.route_previous = Some("claude".into());
        ts.route_note = Some("stall after 2 attempt(s)".into());
        let text = serde_json::to_string(&ts).unwrap();
        assert!(text.contains("\"route_source\":\"failover\""));
        assert!(text.contains("\"route_previous\":\"claude\""));
        let back: TaskState = serde_json::from_str(&text).unwrap();
        assert_eq!(back.route_source, Some(RouteSource::Failover));
        assert_eq!(back.route_previous.as_deref(), Some("claude"));
        assert_eq!(back.route_note.as_deref(), Some("stall after 2 attempt(s)"));
    }

    /// P1-1: pending() must not invent a route_source.
    #[test]
    fn task_state_pending_leaves_route_unset() {
        let ts = TaskState::pending("claude", "print");
        assert!(ts.route_source.is_none());
        assert!(ts.route_previous.is_none());
        assert!(ts.route_note.is_none());
        // unset Option fields stay out of JSON (skip_serializing_if)
        let v: serde_json::Value = serde_json::to_value(&ts).unwrap();
        assert!(v.get("route_source").is_none());
        assert!(v.get("route_previous").is_none());
        assert!(v.get("route_note").is_none());
    }

    /// P1-1: full legacy run.json (no route_*) loads via RunState::load.
    #[test]
    fn run_state_load_legacy_run_json_without_route_fields() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let body = r#"{
  "schema": "cco-run/v1",
  "run_id": "20260721T000000Z-leg1",
  "project_root": "/tmp/proj",
  "plan_path": "/tmp/proj/plan.md",
  "adapter": "cco-plan/v1",
  "started_at": "2026-07-21T00:00:00Z",
  "status": "completed",
  "tasks": {
    "t1": {
      "status": "done",
      "provider": "claude",
      "mode": "print",
      "attempt": 1,
      "failover_used": false
    }
  }
}"#;
        let mut f = std::fs::File::create(run_dir.join("run.json")).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        let rs = RunState::load(run_dir).expect("legacy run.json load");
        assert_eq!(rs.run_id, "20260721T000000Z-leg1");
        assert_eq!(rs.run_dir, run_dir);
        let t1 = rs.tasks.get("t1").expect("t1");
        assert_eq!(t1.status, TaskStatus::Done);
        assert!(t1.route_source.is_none());
        assert!(t1.route_previous.is_none());
        assert!(t1.route_note.is_none());
    }

    /// All RouteSource wire tags are stable snake_case (contract lock).
    #[test]
    fn route_source_wire_values() {
        let cases = [
            (RouteSource::Explicit, "explicit"),
            (RouteSource::SoftFill, "soft_fill"),
            (RouteSource::TagRouting, "tag_routing"),
            (RouteSource::Force, "force"),
            (RouteSource::Failover, "failover"),
            (RouteSource::CostAuto, "cost_auto"),
            (RouteSource::CostEscalate, "cost_escalate"),
            (RouteSource::CostBudget, "cost_budget"),
        ];
        for (src, wire) in cases {
            let s = serde_json::to_string(&src).unwrap();
            assert_eq!(s, format!("\"{wire}\""));
            let back: RouteSource = serde_json::from_str(&s).unwrap();
            assert_eq!(back, src);
        }
    }

    /// Manual card「再跑一次」: only the named failed task resets; Done stays Done.
    #[test]
    fn prepare_task_retry_only_resets_target() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let body = r#"{
  "schema": "cco-run/v1",
  "run_id": "20260722T000000Z-rtry",
  "project_root": "/tmp/proj",
  "plan_path": "/tmp/proj/plan.md",
  "adapter": "cco-plan/v1",
  "started_at": "2026-07-22T00:00:00Z",
  "status": "failed",
  "tasks": {
    "ok": {
      "status": "done",
      "provider": "claude",
      "mode": "print",
      "attempt": 1,
      "failover_used": false
    },
    "bad": {
      "status": "failed",
      "provider": "codex",
      "mode": "print",
      "attempt": 2,
      "failover_used": true,
      "error": "boom",
      "last_retry_reason": "fail"
    }
  }
}"#;
        let mut f = std::fs::File::create(run_dir.join("run.json")).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        let mut rs = RunState::load(run_dir).expect("load");
        rs.prepare_task_retry("bad").expect("retry bad");
        assert_eq!(rs.tasks["ok"].status, TaskStatus::Done);
        assert_eq!(rs.tasks["bad"].status, TaskStatus::Pending);
        assert_eq!(rs.tasks["bad"].attempt, 0);
        assert_eq!(rs.tasks["bad"].last_retry_reason.as_deref(), Some("manual"));
        assert!(!rs.tasks["bad"].failover_used);
        assert!(rs.tasks["bad"].error.is_none());
        assert_eq!(rs.status, RunStatus::Init);
        // Done target must refuse
        assert!(rs.prepare_task_retry("ok").is_err());
        // Missing id
        assert!(rs.prepare_task_retry("nope").is_err());
    }
}
