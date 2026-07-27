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
/// Planner soft-cap leaves room for up to three system post-tasks
/// (inspect / git-push / open-pr).
pub const MAX_TASKS: usize = 23;
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
///
/// Host gate contract is structural — prose PASS/FAIL words in body do **not** count.
pub const INSPECT_SYSTEM_PROMPT: &str = "CCO role=inspect: terminal quality gate, not an implementer. Business tree is READ-ONLY. You may WRITE only under `.cco-out/inspect/**`. Do not edit application source to force a pass. Ledger/map closeout is owned by role=closeout (`sys-closeout`), not by inspect.\n\
\n\
## Host gate contract (must follow exactly)\n\
**Machine SoT (required):** write `.cco-out/inspect/GATE.json` as:\n\
  {\"schema\":\"cco-inspect-gate/v1\",\"result\":\"pass\"|\"fail\",\"blocking\":N,\"map\":N,\"residual\":N}\n\
Host uses GATE.json first. residual does not block pass. Open blocking/map → result fail.\n\
\n\
**Human products (also write):**\n\
VERDICT.md: first structured line `Result: PASS` or `Result: FAIL` (bold ok). Host ignores bare FAIL/PASS in prose.\n\
ISSUES.md: blocks headed `### I-1` / `### R1` / `### issue_id=R1` with line-start `severity: residual|blocking|map|out-of-scope`.\n\
On FAIL, document issues for rework; do not silently rework business code.\n\
\n\
## Usability floor (before checkbox theatre)\n\
Judge whether the product is usable, not only whether plan checkboxes have evidence.\n\
**blocking (must FAIL):** main path unusable; anti-common-sense defaults (e.g. create already marked done); one action mutates other objects; primary save/load loses data; smoke main-path ok:false for functional checks.\n\
**residual only:** handwalk/video/screenshot not recorded, uncommitted hygiene, optional polish, toast timing flicker without wrong state.\n\
Never downgrade unusable / wrong-state issues to residual to force PASS.";
/// Tools that mutate business source — stripped for inspect unless `allow_business_write`.
pub(crate) const INSPECT_STRIP_TOOLS: &[&str] = &["Edit", "MultiEdit", "NotebookEdit"];

/// Marker injected into implement / default-do worker `append_system_prompt` (idempotent).
pub const IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER: &str = "CCO role=implement-usability:";
/// Platform floor for product work: ship usable software, not a demo shell.
///
/// Injected for Implement / Integrate / role-unset business tasks (cco-split `do`
/// often has no role). Scout / Inspect / Closeout are excluded.
pub const IMPLEMENT_USABILITY_SYSTEM_PROMPT: &str = "CCO role=implement-usability: deliver usable software, not a demo shell.\n\
1. Each user action updates only its target object (click A must not rewrite B).\n\
2. Defaults must match the main scenario; if unsure, prefer the safer side that needs user confirmation, and say so in the UI.\n\
3. Action copy ≠ status copy (button \"mark watered\" vs status \"watered today / in N days\").\n\
4. Self-check the main path: create → primary action → refresh still correct; plus one isolation check (only one object changes).\n\
5. Do not weaken behavior or plant fake data just to pass acceptance.\n\
6. Missing assets: search/download from a citable stock library (Unsplash/Pexels/Pixabay) or generate and save under the plan path, then update references. Do not pass \"real-feel product/hero photos\" with only geometric SVG illustrations, and do not rewrite the success criterion to \"no placehold host\" alone.";


/// Fixed id for host-injected Ensure closeout task (E1).
pub const SYS_CLOSEOUT_ID: &str = "sys-closeout";
/// Marker injected into closeout `append_system_prompt` (idempotent).
pub const CLOSEOUT_SYSTEM_PROMPT_MARKER: &str = "CCO role=closeout:";
/// System-prompt segment for closeout workers (bounded docs/ledger write).
pub const CLOSEOUT_SYSTEM_PROMPT: &str = "CCO role=closeout: bounded ledger/map closeout after implement. You may WRITE docs/**, README*, CLAUDE.md, .cco-out/progress/**, tests/**/README* only when evidence (smoke/tests/progress) already supports the checkbox. Never edit business source (src/**, web app code) to force green. Never weaken acceptance criteria. No evidence → do not mark plan items done.";
/// Default writable paths for closeout.
pub const CLOSEOUT_DEFAULT_WRITE_SCOPE: &[&str] = &[
    "docs/**",
    "**/*.md",
    "README*",
    "CLAUDE.md",
    ".cco-out/progress/**",
    ".cco-out/**",
    "tests/**/README*",
];
/// Hard forbid for closeout (business source).
pub const CLOSEOUT_DEFAULT_FORBID: &[&str] =
    &["**/src/**", "src/**", "src-tauri/**", "web/js/**", "web/css/**", "crates/**"];

/// Marker injected when task tags include `browser` (idempotent).
/// Host only injects when `config.browser.enabled` (see browser-automation-cco.md).
pub const BROWSER_SYSTEM_PROMPT_MARKER: &str = "CCO browser-tools:";
/// Discipline for optional browser MCP steps (screenshot / scrape / form smoke).
pub const BROWSER_SYSTEM_PROMPT: &str = "CCO browser-tools: optional browser MCP is available for this task.\n\
1. Prefer URL from env `CCO_PREVIEW_URL` when set; otherwise only URLs named in the task prompt.\n\
2. Write evidence under env `CCO_BROWSER_OUT` (or `.cco-out/browser/<task_id>/`): `shot.png`, `report.md` / `raw.md` / `smoke.md`. Required outputs must exist on disk.\n\
3. Flow: open → wait for main content → screenshot/extract → human conclusion (title, primary CTA, breakage). Missing preview is not PASS — say so in the report.\n\
4. Scrape: record source URL; business writes only in scope.paths; keep a short raw excerpt under the evidence dir.\n\
5. Do not browse unrelated sites, invent screenshots, or bypass cco confirm; browser is a worker tool, not a planner.";

/// Collaboration role for multi-CLI plans (P1-1 + Ensure closeout).
///
/// Serialized as snake_case: `scout` | `implement` | `integrate` | `inspect` | `closeout`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    Scout,
    Implement,
    Integrate,
    Inspect,
    /// Bounded docs/ledger closeout (Ensure E2). Not a second inspect.
    Closeout,
}

impl TaskRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scout => "scout",
            Self::Implement => "implement",
            Self::Integrate => "integrate",
            Self::Inspect => "inspect",
            Self::Closeout => "closeout",
        }
    }

    /// Parse role names (case-insensitive).
    /// Empty / `none` / `auto` / `-` → clear (returns `Ok(None)` when used via
    /// [`parse_role_input`]); this method only accepts known role names.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scout" => Some(Self::Scout),
            "implement" | "impl" => Some(Self::Implement),
            "integrate" | "integration" => Some(Self::Integrate),
            "inspect" | "review" | "check" => Some(Self::Inspect),
            "closeout" | "ledger" | "docs-closeout" => Some(Self::Closeout),
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
        .ok_or_else(|| {
            format!(
                "不支持的角色: {s}（可选 scout / implement / integrate / inspect / closeout，或留空）"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_role_parse_and_as_str() {
        assert_eq!(TaskRole::parse("IMPLEMENT"), Some(TaskRole::Implement));
        assert_eq!(TaskRole::parse("check"), Some(TaskRole::Inspect));
        assert_eq!(TaskRole::parse("closeout"), Some(TaskRole::Closeout));
        assert_eq!(TaskRole::as_str(TaskRole::Scout), "scout");
        assert_eq!(TaskRole::as_str(TaskRole::Closeout), "closeout");
        assert_eq!(parse_role_input("").unwrap(), None);
        assert_eq!(parse_role_input("none").unwrap(), None);
        assert_eq!(
            parse_role_input("integrate").unwrap(),
            Some(TaskRole::Integrate)
        );
        assert_eq!(
            parse_role_input("closeout").unwrap(),
            Some(TaskRole::Closeout)
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
    /// Host shell verify one-liner (H2). Scheduler prefers this over [`Self::acceptance`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_cmd: Option<String>,
    /// Legacy acceptance slot (YAML/plan wire). May still hold shell for old plans;
    /// human prose must not be `sh -c`'d (see `is_runnable_verify` / H0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

impl TaskIR {
    /// Shell command the host may run after the task (H2).
    /// Prefers `verify_cmd`; falls back to `acceptance` only when runnable.
    pub fn effective_verify_cmd(&self) -> Option<&str> {
        if let Some(v) = self
            .verify_cmd
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if super::verify::is_runnable_verify(v) {
                return Some(v);
            }
        }
        if let Some(a) = self
            .acceptance
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if super::verify::is_runnable_verify(a) {
                return Some(a);
            }
        }
        None
    }
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

