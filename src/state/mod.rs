//! Run state on disk: run.json, events.jsonl, per-task files · SQLite dual-write.
//!
//! [INPUT]: runs_root · PlanIR（初始化）
//! [OUTPUT]: RunState/TaskState(attempt/failover_used) · save/load · event append · sqlite
//! [POS]: 运行状态落盘；scheduler 与 services 读写
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/state/CLAUDE.md

pub mod cco_split_store;
pub mod sqlite;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}

impl TaskState {
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
    /// Absolute path to this run directory
    #[serde(skip)]
    pub run_dir: PathBuf,
}

impl RunState {
    pub fn new(
        run_id: String,
        project_root: PathBuf,
        plan: &PlanIR,
        run_dir: PathBuf,
    ) -> Self {
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
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
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
            if matches!(
                ts.status,
                TaskStatus::Done | TaskStatus::Skipped
            ) {
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

    pub fn total_cost_usd(&self) -> f64 {
        self.tasks
            .values()
            .filter_map(|t| t.cost_usd)
            .sum()
    }
}
