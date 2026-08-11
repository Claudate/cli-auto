use std::collections::HashSet;

use anyhow::{bail, Result};

use super::optional::normalize_optional_title;
use super::system_ids::is_system_post_task;
use super::types::{
    PlanIR, TaskIR, TaskRole, IMPLEMENT_USABILITY_SYSTEM_PROMPT,
    IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER, INSPECT_DEFAULT_ALLOWED_TOOLS,
    INSPECT_DEFAULT_WRITE_SCOPE, INSPECT_STRIP_TOOLS, INSPECT_SYSTEM_PROMPT,
    INSPECT_SYSTEM_PROMPT_MARKER,
};
use crate::domain::plan::risk::task_has_browser_tag;
use crate::domain::plan::types::TaskIR; // for helpers

/// Apply per-role default opts / scope after adapter parse (P2-1 + usability floor).
///
/// `role: inspect`:
/// - strip business-mutation tools (`Edit` / `MultiEdit` / `NotebookEdit`) unless
///   `provider_opts.allow_business_write: true`
/// - if `allowed_tools` empty/missing after strip → `INSPECT_DEFAULT_ALLOWED_TOOLS`
/// - ensure Write is present (VERDICT/ISSUES)
/// - empty `scope.paths` → `[.cco-out/inspect/**]`
/// - inject `INSPECT_SYSTEM_PROMPT` into `append_system_prompt` (idempotent)
/// - **empty `depends_on` → wire to business DAG leaves** so terminal inspect
///   never races implement/scout (LLM/split often leave inspect with `[]`)
///
/// Business landers (`Implement` / `Integrate` / **role unset**):
/// - inject `IMPLEMENT_USABILITY_SYSTEM_PROMPT` (idempotent)
/// - cco-split `do` often has no role — still gets the usability floor
///
/// Tags contain `browser`:
/// - inject `BROWSER_SYSTEM_PROMPT` (idempotent). Runtime still gates MCP on config.
///
/// Scout / Closeout / system-post: no implement usability segment.
/// Call sites: [`load_plan`], [`materialize_selected_tasks`].
pub fn materialize_role_defaults(plan: &mut PlanIR) {
    wire_empty_inspect_depends_on(plan);
    for task in &mut plan.tasks {
        if task.role == Some(TaskRole::Inspect) {
            materialize_inspect_task(task);
            // inspect may still carry browser tag for page evidence
            if task_has_browser_tag(&task.tags) {
                inject_browser_system_prompt(&mut task.provider_opts);
            }
            ensure_browser_evidence_outputs(task);
            continue;
        }
        if should_inject_implement_usability(task) {
            inject_implement_usability_system_prompt(&mut task.provider_opts);
        }
        if task_has_browser_tag(&task.tags) {
            inject_browser_system_prompt(&mut task.provider_opts);
        }
        ensure_browser_evidence_outputs(task);
    }
}

/// Default outputs for browser evidence so Done cannot claim success without files.
///
/// - `ui-verify` → shot.png + report.md under `.cco-out/browser/<id>/`
/// - `ui-smoke` → smoke.md (+ shot.png optional, not forced)
/// - `scrape` → raw.md (business write paths stay in author scope)
///
/// Does not remove author-declared outputs; only fills missing defaults.
pub(crate) fn ensure_browser_evidence_outputs(task: &mut TaskIR) {
    use crate::domain::plan::risk::{
        task_has_browser_tag, task_has_scrape_tag, task_has_ui_smoke_tag, task_has_ui_verify_tag,
    };
    if !task_has_browser_tag(&task.tags) {
        return;
    }
    let base = format!(".cco-out/browser/{}", task.id);
    let mut required: Vec<String> = Vec::new();
    if task_has_ui_verify_tag(&task.tags) {
        required.push(format!("{base}/shot.png"));
        required.push(format!("{base}/report.md"));
    }
    if task_has_ui_smoke_tag(&task.tags) {
        required.push(format!("{base}/smoke.md"));
    }
    if task_has_scrape_tag(&task.tags) {
        required.push(format!("{base}/raw.md"));
    }
    // Generic browser tag only (no specialized subtag): still require a report.
    if required.is_empty() {
        required.push(format!("{base}/report.md"));
    }
    for rel in required {
        if !task.outputs.iter().any(|o| o == &rel) {
            task.outputs.push(rel);
        }
    }
    // Ensure evidence dir is writable in scope when scope exists or role=implement.
    if let Some(scope) = task.scope.as_mut() {
        let glob = format!("{base}/**");
        if !scope
            .paths
            .iter()
            .any(|p| p == &glob || p == ".cco-out/browser/**")
        {
            scope.paths.push(glob);
        }
    }
}

/// Business landers that ship product behavior (not scout/inspect/closeout/system-post).
pub(crate) fn should_inject_implement_usability(task: &TaskIR) -> bool {
    if is_system_post_task(&task.id) {
        return false;
    }
    match task.role {
        Some(TaskRole::Inspect) | Some(TaskRole::Closeout) | Some(TaskRole::Scout) => false,
        // Implement, Integrate, or role-unset (common for cco-split `do`).
        Some(TaskRole::Implement) | Some(TaskRole::Integrate) | None => true,
    }
}

/// Business leaves = non-inspect, non-closeout, non-system-post tasks that no
/// *other* business task lists in `depends_on` (DAG sinks of implement/scout work).
///
/// Inspect with empty `depends_on` waits on those leaves (transitively covers the
/// whole business wave). Explicit inspect edges are left alone.
/// Closeout is host-owned Ensure and is not a business leaf.
pub(crate) fn wire_empty_inspect_depends_on(plan: &mut PlanIR) {
    let business_ids: Vec<String> = plan
        .tasks
        .iter()
        .filter(|t| {
            t.role != Some(TaskRole::Inspect)
                && t.role != Some(TaskRole::Closeout)
                && !is_system_post_task(&t.id)
        })
        .map(|t| t.id.clone())
        .collect();
    if business_ids.is_empty() {
        return;
    }

    let mut is_predecessor: HashSet<String> = HashSet::new();
    for t in &plan.tasks {
        if t.role == Some(TaskRole::Inspect)
            || t.role == Some(TaskRole::Closeout)
            || is_system_post_task(&t.id)
        {
            continue;
        }
        for d in &t.depends_on {
            is_predecessor.insert(d.clone());
        }
    }
    let mut leaves: Vec<String> = business_ids
        .iter()
        .filter(|id| !is_predecessor.contains(*id))
        .cloned()
        .collect();
    leaves.sort();
    leaves.dedup();

    for t in &mut plan.tasks {
        if t.role == Some(TaskRole::Inspect) {
            for leaf in &leaves {
                if !t.depends_on.contains(leaf) {
                    t.depends_on.push(leaf.clone());
                }
            }
        }
    }
}

// More helpers would go here if needed. The rest of the original file is now in tests or cleaned.
