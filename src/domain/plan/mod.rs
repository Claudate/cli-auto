//! Domain plan model (A1 · P2-17).
//!
//! Pure types, validation, materialize, tag routing — no filesystem, no Config IO.
//! Adapters / load_plan / planner stay in `crate::plan` (migration facade).
//!
//! [INPUT]: 无 IO
//! [OUTPUT]: PlanIR 与纯函数
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部与 src/domain/CLAUDE.md

pub mod cco_split;
mod checklist;
mod closeout;
mod materialize;
mod merge_check;
mod optional;
mod risk;
mod routing;
mod soften;
mod system_ids;
mod types;
mod validate;
mod verify;

pub use cco_split::{
    from_plan_ir, recompute_waves, run_gate_ok, sanitize_cco_split_deps, soft_accept_split,
    split_topo_layers, to_plan_ir, CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask,
    CcoTaskKind, CcoTaskStatus, CCO_SPLIT_SCHEMA,
};
pub use checklist::{
    assign_closeout_owners, build_host_checklist, format_checklist_for_prompt, ChecklistKind,
    HostChecklist, HostChecklistItem, CHECKLIST_SCHEMA_VERSION,
};
pub use closeout::{
    inject_closeout_task, inspect_has_closeout_duty, looks_like_inspect_gate, should_inject_closeout,
    strip_inspect_closeout_duty,
};
pub use materialize::{materialize_role_defaults, materialize_selected_tasks};
pub use merge_check::{
    humanize_soft_accept_note, merge_check_for_integrate, merge_check_for_plan,
    soft_accept_human_tips, MERGE_CHECK_DEFAULT,
};
pub use optional::{
    looks_like_work_task_id, normalize_optional_title, title_is_meta_heading, title_looks_optional,
};
pub use risk::{classify_task_risk, classify_task_risk_wire, RiskClass};
pub use routing::{apply_tag_routing, tag_implied_provider};
pub use soften::soften_plan_for_accept;
pub use system_ids::{
    is_system_closeout_task, is_system_ensure_task, is_system_post_task, SYS_POST_GIT_PUSH_ID,
    SYS_POST_INSPECT_ID, SYS_POST_OPEN_PR_ID,
};
pub use types::{
    parse_role_input, OnFailure, PlanIR, TaskIR, TaskRole, TaskScope, CLOSEOUT_DEFAULT_FORBID,
    CLOSEOUT_DEFAULT_WRITE_SCOPE, CLOSEOUT_SYSTEM_PROMPT, CLOSEOUT_SYSTEM_PROMPT_MARKER,
    IMPLEMENT_USABILITY_SYSTEM_PROMPT, IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER,
    INSPECT_DEFAULT_ALLOWED_TOOLS, INSPECT_DEFAULT_WRITE_SCOPE, INSPECT_SYSTEM_PROMPT,
    INSPECT_SYSTEM_PROMPT_MARKER, MAX_PROMPT_CHARS, MAX_TASKS, MAX_TIMEOUT_SECS,
    PLANNER_MAX_BUDGET_USD, PLANNER_MAX_TASKS, SYS_CLOSEOUT_ID,
};
pub use verify::{is_runnable_verify, looks_like_shell_acceptance};

// Crate-private helpers (soft_accept uses validate::* via super::super; tests need re-export).
#[cfg(test)]
pub(crate) use materialize::materialize_inspect_task;
#[cfg(test)]
pub(crate) use validate::{scope_glob_prefix, scope_paths_overlap};
