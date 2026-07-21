//! System post-task ids (pure domain).
//! Host inject lives in `plan::system_post`; validate only needs id checks.

/// Fixed id — task 巡检（对照计划勾选 / VERDICT+ISSUES）.
pub const SYS_POST_INSPECT_ID: &str = "sys-post-inspect";
/// Fixed id — git commit + push（可选收尾）.
pub const SYS_POST_GIT_PUSH_ID: &str = "sys-post-git-push";
/// Fixed id — open GitHub PR via `gh`（可选收尾 · S-PR / P2-7）.
pub const SYS_POST_OPEN_PR_ID: &str = "sys-post-open-pr";

/// True when task id is a host-owned system post-task.
pub fn is_system_post_task(id: &str) -> bool {
    id == SYS_POST_INSPECT_ID
        || id == SYS_POST_GIT_PUSH_ID
        || id == SYS_POST_OPEN_PR_ID
}
