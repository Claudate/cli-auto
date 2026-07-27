//! Materialize selected optionals + role defaults (pure, in-memory).
//!
//! [INPUT]: PlanIR
//! [OUTPUT]: materialize_selected_tasks · materialize_role_defaults
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部；inspect 空 depends_on 接线变更须同步单测

use std::collections::HashSet;

use anyhow::{bail, Result};

use super::system_ids::is_system_post_task;
use super::types::{
    PlanIR, TaskIR, TaskRole, IMPLEMENT_USABILITY_SYSTEM_PROMPT,
    IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER, INSPECT_DEFAULT_ALLOWED_TOOLS,
    INSPECT_DEFAULT_WRITE_SCOPE, INSPECT_SYSTEM_PROMPT, INSPECT_SYSTEM_PROMPT_MARKER,
    INSPECT_STRIP_TOOLS,
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
            continue;
        }
        if should_inject_implement_usability(task) {
            inject_implement_usability_system_prompt(&mut task.provider_opts);
        }
        if super::risk::task_has_browser_tag(&task.tags) {
            inject_browser_system_prompt(&mut task.provider_opts);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::{OnFailure, TaskScope};

    fn task(id: &str, role: Option<TaskRole>, deps: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: Some(TaskScope {
                paths: vec![format!(".cco-out/{id}/**")],
                readonly: vec![],
                forbid: vec![],
            }),
            outputs: vec![],
            tags: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "test".into(),
            source_path: std::path::PathBuf::from("test.md"),
            max_parallel: 4,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn empty_inspect_depends_wires_to_business_leaves() {
        // t1 → t2, t3; leaves = t2,t3; inspect had []
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Scout), &[]),
            task("t2", Some(TaskRole::Implement), &["t1"]),
            task("t3", Some(TaskRole::Implement), &["t1"]),
            task("t7-inspect", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        assert!(
            insp.depends_on.iter().any(|d| d == "t2"),
            "deps={:?}",
            insp.depends_on
        );
        assert!(insp.depends_on.iter().any(|d| d == "t3"));
        assert!(!insp.depends_on.iter().any(|d| d == "t1"), "only leaves");
    }

    #[test]
    fn explicit_inspect_depends_preserved() {
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Implement), &[]),
            task("t2", Some(TaskRole::Implement), &[]),
            task("t7-inspect", Some(TaskRole::Inspect), &["t1"]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        assert_eq!(insp.depends_on, vec!["t1".to_string()]);
    }

    #[test]
    fn docs_cleanup_shape_inspect_not_parallel_to_t1() {
        // Real failure shape: t7-inspect [] raced t1-inventory at run start.
        let mut ir = plan(vec![
            task("t1-inventory", Some(TaskRole::Scout), &[]),
            task("t2-delete-one", Some(TaskRole::Implement), &["t1-inventory"]),
            task("t3-archive-b", Some(TaskRole::Implement), &["t1-inventory"]),
            task(
                "t4-c1-split-merge",
                Some(TaskRole::Implement),
                &["t3-archive-b"],
            ),
            task(
                "t5-c2c3c4-light",
                Some(TaskRole::Implement),
                &["t3-archive-b"],
            ),
            task(
                "t6-index-refresh",
                Some(TaskRole::Integrate),
                &["t3-archive-b"],
            ),
            task("t7-inspect", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        for leaf in ["t2-delete-one", "t4-c1-split-merge", "t5-c2c3c4-light", "t6-index-refresh"]
        {
            assert!(
                insp.depends_on.iter().any(|d| d == leaf),
                "missing leaf {leaf}; deps={:?}",
                insp.depends_on
            );
        }
        assert!(!insp.depends_on.iter().any(|d| d == "t1-inventory"));
        assert!(!insp.depends_on.iter().any(|d| d == "t3-archive-b"));
    }

    fn sys_of(task: &TaskIR) -> String {
        task.provider_opts
            .get("append_system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn implement_and_role_unset_get_usability_floor() {
        let mut ir = plan(vec![
            task("t-impl", Some(TaskRole::Implement), &[]),
            task("t-do", None, &[]),
            task("t-int", Some(TaskRole::Integrate), &[]),
            task("t-scout", Some(TaskRole::Scout), &[]),
            task("t-insp", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);

        for id in ["t-impl", "t-do", "t-int"] {
            let t = ir.tasks.iter().find(|x| x.id == id).unwrap();
            let sys = sys_of(t);
            assert!(
                sys.contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
                "{id} missing usability floor: {sys}"
            );
            assert!(
                !sys.contains(INSPECT_SYSTEM_PROMPT_MARKER),
                "{id} must not get inspect prompt"
            );
        }

        let scout = ir.tasks.iter().find(|x| x.id == "t-scout").unwrap();
        assert!(
            !sys_of(scout).contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
            "scout must not get implement usability"
        );

        let insp = ir.tasks.iter().find(|x| x.id == "t-insp").unwrap();
        let insp_sys = sys_of(insp);
        assert!(insp_sys.contains(INSPECT_SYSTEM_PROMPT_MARKER));
        assert!(
            insp_sys.contains("Usability floor"),
            "inspect prompt should carry usability severity floor"
        );
        assert!(
            !insp_sys.contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
            "inspect must not get implement-usability marker"
        );
    }

    #[test]
    fn implement_usability_inject_is_idempotent() {
        let mut ir = plan(vec![task("t1", Some(TaskRole::Implement), &[])]);
        materialize_role_defaults(&mut ir);
        materialize_role_defaults(&mut ir);
        let sys = sys_of(ir.tasks.iter().find(|t| t.id == "t1").unwrap());
        assert_eq!(
            sys.matches(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER).count(),
            1,
            "usability marker duplicated: {sys}"
        );
    }

    #[test]
    fn browser_tag_injects_browser_prompt_idempotent() {
        use super::super::types::BROWSER_SYSTEM_PROMPT_MARKER;
        let mut t = task("ui", Some(TaskRole::Implement), &[]);
        t.tags = vec!["browser".into(), "ui-verify".into()];
        let mut ir = plan(vec![t]);
        materialize_role_defaults(&mut ir);
        materialize_role_defaults(&mut ir);
        let sys = sys_of(ir.tasks.iter().find(|x| x.id == "ui").unwrap());
        assert!(
            sys.contains(BROWSER_SYSTEM_PROMPT_MARKER),
            "missing browser prompt: {sys}"
        );
        assert_eq!(
            sys.matches(BROWSER_SYSTEM_PROMPT_MARKER).count(),
            1,
            "browser marker duplicated: {sys}"
        );
    }
}

