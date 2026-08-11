//! Materialize selected optionals + role defaults (pure, in-memory).
//!
//! [INPUT]: PlanIR
//! [OUTPUT]: materialize_selected_tasks · materialize_role_defaults
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部；inspect 空 depends_on 接线变更须同步单测

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

/// Drop unselected optional tasks and rewrite depends_on for execution.
pub fn materialize_selected_tasks(mut ir: PlanIR) -> Result<PlanIR> {
    for t in &mut ir.tasks {
        if !t.optional {
            t.include = true;
        }
        if t.optional {
            t.title = normalize_optional_title(&t.title, true);
        }
    }
    let drop: HashSet<String> = ir
        .tasks
        .iter()
        .filter(|t| !t.include)
        .map(|t| t.id.clone())
        .collect();
    ir.tasks.retain(|t| t.include);
    if ir.tasks.is_empty() {
        bail!("没有选中任何任务：请至少保留一项必选，或勾选可选项后再开始");
    }
    for t in &mut ir.tasks {
        t.depends_on.retain(|d| !drop.contains(d));
    }
    // Role defaults already applied at load_plan; re-apply is idempotent if IR
    // was built in-memory (planner / tests) without going through load_plan.
    materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}

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
            if super::risk::task_has_browser_tag(&task.tags) {
                inject_browser_system_prompt(&mut task.provider_opts);
            }
            ensure_browser_evidence_outputs(task);
            continue;
        }
        if should_inject_implement_usability(task) {
            inject_implement_usability_system_prompt(&mut task.provider_opts);
        }
        if super::risk::task_has_browser_tag(&task.tags) {
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
    use super::risk::{
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
    if leaves.is_empty() {
        // Degenerate cycle or all intermediate — fall back to every business task.
        leaves = business_ids;
    }
    leaves.sort();

    for t in plan.tasks.iter_mut() {
        if t.role != Some(TaskRole::Inspect) {
            continue;
        }
        if !t.depends_on.is_empty() {
            continue;
        }
        t.depends_on = leaves.clone();
    }
}

pub(crate) fn materialize_inspect_task(task: &mut TaskIR) {
    let allow_business_write = task
        .provider_opts
        .get("allow_business_write")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ── allowed_tools (Claude / fake; codex ignores) ─────────────────
    if !allow_business_write {
        let tools = normalize_inspect_allowed_tools(task.provider_opts.get("allowed_tools"));
        task.provider_opts["allowed_tools"] = serde_json::json!(tools);
    }

    // ── scope.paths writable whitelist ───────────────────────────────
    let need_default_paths = match task.scope.as_ref() {
        None => true,
        Some(s) => s.paths.is_empty(),
    };
    if need_default_paths {
        let mut scope = task.scope.take().unwrap_or_default();
        scope.paths = vec![INSPECT_DEFAULT_WRITE_SCOPE.to_string()];
        task.scope = Some(scope);
    }

    // Prefer GATE.json when the plan already lists inspect products; do **not**
    // hard-require GATE as missing_outputs (legacy VERDICT-only runs must still gate).
    // Prompt + host prefer GATE when present (see inspect_io::load_inspect_gate_doc).
    use crate::domain::inspect::{INSPECT_GATE_REL, INSPECT_ISSUES_REL, INSPECT_VERDICT_REL};
    let has_inspect_product = task.outputs.iter().any(|o| {
        let l = o.to_ascii_lowercase();
        l.contains("verdict") || l.contains("issues") || l.contains("gate.json")
    }) || task.role == Some(TaskRole::Inspect);
    if has_inspect_product {
        for req in [INSPECT_VERDICT_REL, INSPECT_ISSUES_REL] {
            if !task.outputs.iter().any(|o| o == req) {
                task.outputs.push(req.into());
            }
        }
        // Soft list GATE so workers know the path; host does not fail missing GATE.
        if !task.outputs.iter().any(|o| o == INSPECT_GATE_REL) {
            // Keep out of hard outputs — document only in prompt.
        }
    }

    // ── system prompt segment ────────────────────────────────────────
    inject_inspect_system_prompt(&mut task.provider_opts, allow_business_write);
}

pub(crate) fn normalize_inspect_allowed_tools(raw: Option<&serde_json::Value>) -> Vec<String> {
    let mut tools: Vec<String> = match raw {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .filter(|t| {
                !INSPECT_STRIP_TOOLS
                    .iter()
                    .any(|b| t.eq_ignore_ascii_case(b))
            })
            .collect(),
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|t| {
                !t.is_empty()
                    && !INSPECT_STRIP_TOOLS
                        .iter()
                        .any(|b| t.eq_ignore_ascii_case(b))
            })
            .collect(),
        _ => Vec::new(),
    };

    if tools.is_empty() {
        return INSPECT_DEFAULT_ALLOWED_TOOLS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }

    // Ensure report write + basic read are always available for the gate.
    for required in ["Read", "Write"] {
        if !tools.iter().any(|t| t.eq_ignore_ascii_case(required)) {
            tools.push(required.to_string());
        }
    }
    tools
}

pub(crate) fn inject_inspect_system_prompt(
    opts: &mut serde_json::Value,
    allow_business_write: bool,
) {
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(INSPECT_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let segment = if allow_business_write {
        format!(
            "{INSPECT_SYSTEM_PROMPT} Note: allow_business_write=true — prefer still writing only under `.cco-out/inspect/**`; do not silently rework features."
        )
    } else {
        INSPECT_SYSTEM_PROMPT.to_string()
    };
    let merged = if existing.trim().is_empty() {
        segment
    } else {
        format!("{existing}\n\n{segment}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

/// Inject implement usability floor into `append_system_prompt` (idempotent).
pub(crate) fn inject_implement_usability_system_prompt(opts: &mut serde_json::Value) {
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let merged = if existing.trim().is_empty() {
        IMPLEMENT_USABILITY_SYSTEM_PROMPT.to_string()
    } else {
        format!("{existing}\n\n{IMPLEMENT_USABILITY_SYSTEM_PROMPT}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

/// Inject browser MCP discipline (idempotent). Gated at runtime by config.enabled.
pub(crate) fn inject_browser_system_prompt(opts: &mut serde_json::Value) {
    use super::types::{BROWSER_SYSTEM_PROMPT, BROWSER_SYSTEM_PROMPT_MARKER};
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(BROWSER_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let merged = if existing.trim().is_empty() {
        BROWSER_SYSTEM_PROMPT.to_string()
    } else {
        format!("{existing}\n\n{BROWSER_SYSTEM_PROMPT}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

