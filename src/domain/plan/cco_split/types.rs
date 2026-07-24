//! CcoSplit pure types (no convert / accept logic).
//!
//! [INPUT]: 无 IO
//! [OUTPUT]: CcoSplitJob/Task enums + structs
//! [POS]: domain/plan/cco_split
//! [PROTOCOL]: 变更时更新此头部

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Schema id for cco split docs (JSON export / future wire).
pub const CCO_SPLIT_SCHEMA: &str = "cco-split/v1";

/// How the split graph was produced.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CcoSplitSource {
    #[default]
    Heuristic,
    Llm,
    Merge,
    Manual,
    Parse,
    Fake,
    Import,
}

impl CcoSplitSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Heuristic => "heuristic",
            Self::Llm => "llm",
            Self::Merge => "merge",
            Self::Manual => "manual",
            Self::Parse => "parse",
            Self::Fake => "fake",
            Self::Import => "import",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "llm" | "ai" => Self::Llm,
            "merge" => Self::Merge,
            "manual" => Self::Manual,
            "parse" => Self::Parse,
            "fake" => Self::Fake,
            "import" => Self::Import,
            _ => Self::Heuristic,
        }
    }
}

/// Job-level lifecycle for the split desk (not run status).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CcoSplitStatus {
    #[default]
    Drafting,
    Ready,
    Confirmed,
    Failed,
    Cancelled,
}

impl CcoSplitStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drafting => "drafting",
            Self::Ready => "ready",
            Self::Confirmed => "confirmed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "ready" | "planned" => Self::Ready,
            "confirmed" => Self::Confirmed,
            "failed" | "plan_failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Drafting,
        }
    }
}

/// Step kind for display badges (do / check / system).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CcoTaskKind {
    #[default]
    Do,
    Check,
    System,
}

impl CcoTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Do => "do",
            Self::Check => "check",
            Self::System => "system",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "check" | "inspect" | "review" => Self::Check,
            "system" | "sys" | "post" => Self::System,
            _ => Self::Do,
        }
    }
}

/// Per-task progress after confirm (desk may show pending until run syncs).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CcoTaskStatus {
    #[default]
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

impl CcoTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "running" => Self::Running,
            "done" | "completed" | "success" => Self::Done,
            "failed" | "error" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Pending,
        }
    }
}

/// One split job (desk session) — memory shape; SQLite is SoT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CcoSplitJob {
    pub job_id: String,
    pub project: PathBuf,
    pub plan_path: PathBuf,
    pub status: CcoSplitStatus,
    pub title: String,
    pub max_parallel: usize,
    pub source: CcoSplitSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub tasks: Vec<CcoSplitTask>,
}

/// One step on the split desk + later AI run body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CcoSplitTask {
    pub task_id: String,
    /// List order (display).
    pub ord: i32,
    pub title: String,
    /// Card one-liner.
    #[serde(default)]
    pub summary: String,
    /// Full instruction for the execute worker (maps → PlanIR.prompt).
    pub body: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Concurrent wave (0-based); recomputed from depends when soft-accepting.
    #[serde(default)]
    pub wave: i32,
    /// User checkbox — maps → PlanIR.include.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub optional: bool,
    /// Done criteria (display + inspect) — human only; never `sh -c`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_when: Option<String>,
    /// Optional host shell one-liner (H2). Empty on main path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_cmd: Option<String>,
    /// Plan section / id reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_ref: Option<String>,
    #[serde(default)]
    pub kind: CcoTaskKind,
    #[serde(default)]
    pub status: CcoTaskStatus,
    /// Advanced routing — empty on main path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_paths: Vec<String>,
    /// Extension bag (group, tags, mode, …) — avoid schema churn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_json: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}
