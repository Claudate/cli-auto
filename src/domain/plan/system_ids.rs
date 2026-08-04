//! System post-task ids (pure domain).
//! Host inject lives in `plan::system_post`; validate only needs id checks.

use super::types::SYS_CLOSEOUT_ID;

/// Fixed id — task 巡检（对照计划勾选 / VERDICT+ISSUES）.
pub const SYS_POST_INSPECT_ID: &str = "sys-post-inspect";
/// Fixed id — git commit + push（可选收尾）.
pub const SYS_POST_GIT_PUSH_ID: &str = "sys-post-git-push";
/// Fixed id — open GitHub PR via `gh`（可选收尾 · S-PR / P2-7）.
pub const SYS_POST_OPEN_PR_ID: &str = "sys-post-open-pr";

/// True when task id is a host-owned system post-task (inspect/push/pr).
/// Note: `sys-closeout` is host-injected Ensure closeout but **not** a post-feature
/// toggle task — use [`is_system_ensure_task`] for closeout too.
pub fn is_system_post_task(id: &str) -> bool {
    id == SYS_POST_INSPECT_ID || id == SYS_POST_GIT_PUSH_ID || id == SYS_POST_OPEN_PR_ID
}

/// True for host Ensure closeout id.
pub fn is_system_closeout_task(id: &str) -> bool {
    id == SYS_CLOSEOUT_ID
}

/// Post-feature system tasks **or** Ensure closeout.
pub fn is_system_ensure_task(id: &str) -> bool {
    is_system_post_task(id) || is_system_closeout_task(id)
}
