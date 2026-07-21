//! Domain plan model (A1 · P2-17).
//!
//! Pure types, validation, materialize, tag routing — no filesystem, no Config IO.
//! Adapters / load_plan / planner stay in `crate::plan` (migration facade).
//!
//! [INPUT]: 无 IO
//! [OUTPUT]: PlanIR 与纯函数
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部与 src/domain/CLAUDE.md

mod materialize;
mod optional;
mod routing;
mod system_ids;
mod types;
mod validate;

pub use materialize::{materialize_role_defaults, materialize_selected_tasks};
pub use optional::{normalize_optional_title, title_is_meta_heading, title_looks_optional};
pub use routing::apply_tag_routing;
pub use system_ids::{is_system_post_task, SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID};
pub use types::{
    OnFailure, PlanIR, TaskIR, TaskRole, TaskScope, INSPECT_DEFAULT_ALLOWED_TOOLS,
    INSPECT_DEFAULT_WRITE_SCOPE, INSPECT_SYSTEM_PROMPT, INSPECT_SYSTEM_PROMPT_MARKER,
    MAX_PROMPT_CHARS, MAX_TASKS, MAX_TIMEOUT_SECS, PLANNER_MAX_BUDGET_USD, PLANNER_MAX_TASKS,
};

// Crate-private helpers used by `plan` facade unit tests (A1 migration).
#[cfg(test)]
pub(crate) use materialize::materialize_inspect_task;
#[cfg(test)]
pub(crate) use validate::{scope_glob_prefix, scope_paths_overlap};
