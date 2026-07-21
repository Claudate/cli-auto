//! Materialize selected optionals + role defaults (pure, in-memory).
//!
//! [INPUT]: PlanIR
//! [OUTPUT]: materialize_selected_tasks · materialize_role_defaults
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部

use std::collections::HashSet;

use anyhow::{bail, Result};

use super::types::{
    PlanIR, TaskIR, TaskRole, INSPECT_DEFAULT_ALLOWED_TOOLS, INSPECT_DEFAULT_WRITE_SCOPE,
    INSPECT_SYSTEM_PROMPT, INSPECT_SYSTEM_PROMPT_MARKER, INSPECT_STRIP_TOOLS,
};
use super::optional::normalize_optional_title;

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

/// Apply per-role default opts / scope after adapter parse (P2-1).
///
/// Currently only `role: inspect`:
/// - strip business-mutation tools (`Edit` / `MultiEdit` / `NotebookEdit`) unless
///   `provider_opts.allow_business_write: true`
/// - if `allowed_tools` empty/missing after strip → `INSPECT_DEFAULT_ALLOWED_TOOLS`
/// - ensure Write is present (VERDICT/ISSUES)
/// - empty `scope.paths` → `[.cco-out/inspect/**]`
/// - inject `INSPECT_SYSTEM_PROMPT` into `append_system_prompt` (idempotent)
///
/// Non-inspect roles and plans without `role` are untouched (legacy back-compat).
/// Call sites: [`load_plan`], [`materialize_selected_tasks`].
pub fn materialize_role_defaults(plan: &mut PlanIR) {
    for task in &mut plan.tasks {
        if task.role == Some(TaskRole::Inspect) {
            materialize_inspect_task(task);
        }
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

pub(crate) fn inject_inspect_system_prompt(opts: &mut serde_json::Value, allow_business_write: bool) {
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

