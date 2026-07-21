//! Plan domain types and limits (A1 extracted from plan/mod.rs).
//!
//! [INPUT]: 无 IO
//! [OUTPUT]: PlanIR/TaskIR/TaskRole/TaskScope/OnFailure · MAX_* · INSPECT_*
//! [POS]: domain/plan；纯模型
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/domain/CLAUDE.md

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Product hard limits (P1-4 / B3) ───────────────────────────────────
/// Max tasks in one plan (planner + validate).
/// Planner still targets ≤18 work tasks so two system post-tasks can fit.
pub const MAX_TASKS: usize = 22;
/// Soft cap for planner-produced tasks (system post-tasks use the remainder).
pub const PLANNER_MAX_TASKS: usize = 20;
/// Max characters per task prompt.
pub const MAX_PROMPT_CHARS: usize = 32_000;
/// Max per-task timeout (24h).
pub const MAX_TIMEOUT_SECS: u64 = 86_400;
/// Default planner LLM budget USD (opts; not a validate hard limit).
pub const PLANNER_MAX_BUDGET_USD: f64 = 2.0;

// ── P2-1 role=inspect defaults ───────────────────────────────────────
/// Default Claude tools for `role: inspect` (read + shell + write reports only).
/// No Edit/MultiEdit — inspect is a quality gate, not an implementer (N6).
pub const INSPECT_DEFAULT_ALLOWED_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Bash", "Write"];
/// Default writable whitelist when inspect omits `scope.paths`.
pub const INSPECT_DEFAULT_WRITE_SCOPE: &str = ".cco-out/inspect/**";
/// Marker injected into `provider_opts.append_system_prompt` (idempotent).
pub const INSPECT_SYSTEM_PROMPT_MARKER: &str = "CCO role=inspect:";
/// System-prompt segment for inspect workers (Claude append-system-prompt / host).
pub const INSPECT_SYSTEM_PROMPT: &str = "CCO role=inspect: terminal quality gate, not an implementer. Business tree is READ-ONLY. You may WRITE only under `.cco-out/inspect/**` (VERDICT.md, ISSUES.md, etc.). Do not edit application source to force a pass. On FAIL, document issues for a future rework wave; do not silently rework.";
/// Tools that mutate business source — stripped for inspect unless `allow_business_write`.
pub(crate) const INSPECT_STRIP_TOOLS: &[&str] = &["Edit", "MultiEdit", "NotebookEdit"];

/// Collaboration role for multi-CLI plans (P1-1).
///
/// Serialized as snake_case: `scout` | `implement` | `integrate` | `inspect`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    Scout,
    Implement,
    Integrate,
    Inspect,
}

impl TaskRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scout => "scout",
            Self::Implement => "implement",
            Self::Integrate => "integrate",
            Self::Inspect => "inspect",
        }
    }

    /// Parse `scout|implement|integrate|inspect` (case-insensitive).
    /// Empty / `none` / `auto` / `-` → clear (returns `Ok(None)` when used via
    /// [`parse_role_input`]); this method only accepts known role names.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scout" => Some(Self::Scout),
            "implement" | "impl" => Some(Self::Implement),
            "integrate" | "integration" => Some(Self::Integrate),
            "inspect" | "review" | "check" => Some(Self::Inspect),
            _ => None,
        }
    }
}

/// Parse confirm-screen role input: known role, or clear tokens → `Ok(None)`.
/// Unknown non-empty values → `Err`.
pub fn parse_role_input(raw: &str) -> Result<Option<TaskRole>, String> {
    let s = raw.trim();
    if s.is_empty()
        || matches!(
            s.to_ascii_lowercase().as_str(),
            "none" | "auto" | "-" | "clear" | "默认" | "自动"
        )
    {
        return Ok(None);
    }
    TaskRole::parse(s)
        .map(Some)
        .ok_or_else(|| format!("不支持的角色: {s}（可选 scout / implement / integrate / inspect，或留空）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_role_parse_and_as_str() {
        assert_eq!(TaskRole::parse("IMPLEMENT"), Some(TaskRole::Implement));
        assert_eq!(TaskRole::parse("check"), Some(TaskRole::Inspect));
        assert_eq!(TaskRole::as_str(TaskRole::Scout), "scout");
        assert_eq!(parse_role_input("").unwrap(), None);
        assert_eq!(parse_role_input("none").unwrap(), None);
        assert_eq!(
            parse_role_input("integrate").unwrap(),
            Some(TaskRole::Integrate)
        );
        assert!(parse_role_input("wizard").is_err());
    }
}

/// Per-task path contract (P1-1). All globs relative to project/worktree root.
///
/// - `paths`: writable whitelist (implement/integrate should set)
/// - `readonly`: extra readable ranges (scout may leave empty = full project)
/// - `forbid`: hard deny list
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskScope {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub readonly: Vec<String>,
    #[serde(default)]
    pub forbid: Vec<String>,
}

/// Resolved plan host understands (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanIR {
    pub schema: String,
    pub name: String,
    pub adapter: String,
    pub source_path: PathBuf,
    pub max_parallel: usize,
    pub on_failure: OnFailure,
    pub retry_max: u32,
    pub default_provider: String,
    pub default_mode: String,
    pub worktree: bool,
    /// When true, later validate (P1-2) may require a terminal `role: inspect` task.
    /// Absent in old plans → false (serde default).
    #[serde(default)]
    pub require_inspect: bool,
    pub tasks: Vec<TaskIR>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskIR {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub group: Option<String>,
    pub provider: String,
    /// print | bg | auto
    pub mode: String,
    pub prompt: String,
    pub acceptance: Option<String>,
    pub timeout_secs: Option<u64>,
    pub worktree: Option<bool>,
    /// Opaque to host; validated by provider.
    pub provider_opts: serde_json::Value,
    /// When true, task is optional — confirm screen lets the user opt in/out.
    /// Title should contain 「（可选）」so the choice is obvious in lists.
    #[serde(default)]
    pub optional: bool,
    /// Whether to run this task. Missing field deserializes as false;
    /// `materialize_selected_tasks` forces required tasks on and drops
    /// unselected optional ones. Optional tasks stay off until the user checks.
    #[serde(default)]
    pub include: bool,
    /// Collaboration role (scout|implement|integrate|inspect). Optional for back-compat.
    #[serde(default)]
    pub role: Option<TaskRole>,
    /// Writable / readonly / forbid path globs. Optional for back-compat.
    #[serde(default)]
    pub scope: Option<TaskScope>,
    /// Required on-disk artifact paths after the task completes (relative to project).
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Free-form tags for L1 routing (P2-4): e.g. `codex`, `inspect`, `frontend`.
    /// Absent in old plans → empty (serde default).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    Pause,
    Continue,
    Retry,
}

impl Default for OnFailure {
    fn default() -> Self {
        Self::Pause
    }
}

