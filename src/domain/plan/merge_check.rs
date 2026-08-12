//! Human one-liner: how to verify after parallel / integrate steps (H3).
//!
//! [INPUT]: task role · outputs · whether graph has integrate
//! [OUTPUT]: merge_check 浅白一句；禁止默认写死 MERGE.md
//! [POS]: domain/plan · 纯函数；Presentation 只渲染
//! [PROTOCOL]: 变更时更新此头部 · human-status-verify-dual H3

use crate::domain::plan::types::{TaskIR, TaskRole};

/// Default sentence when there is no integrate step (still honest about partial fail).
pub const MERGE_CHECK_DEFAULT: &str =
    "可以一起干的步骤都完成后，再对照各步说明与计划验收；有一步失败，先别当全部成功";

/// Build merge_check for a plan graph.
///
/// - No integrate/inspect-style join → generic default (or None if caller prefers hide)
/// - Has integrate → mention integrate outputs only when concrete paths exist
/// - Never invent `MERGE.md`
pub fn merge_check_for_plan(tasks: &[TaskIR]) -> Option<String> {
    let integrate = tasks.iter().find(|t| t.role == Some(TaskRole::Integrate));
    if let Some(t) = integrate {
        return Some(merge_check_for_integrate(t));
    }
    // Parallel-ish graph (any multi-dep or multiple roots) still gets the generic tip.
    if tasks.len() >= 2 {
        return Some(MERGE_CHECK_DEFAULT.to_string());
    }
    None
}

/// Integrate-task-specific wording; name paths only when outputs non-empty.
pub fn merge_check_for_integrate(task: &TaskIR) -> String {
    let paths: Vec<&str> = task
        .outputs
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if paths.is_empty() {
        "拼在一起怎么验：先看整合步骤的产出（见该步说明），再跑巡检对照计划".into()
    } else {
        let listed = paths.join("、");
        format!("拼在一起怎么验：先看整合产出（{listed}），再跑巡检对照计划")
    }
}

/// Project soft_accept machine notes → user-facing Chinese when about scope serialize.
pub fn humanize_soft_accept_note(note: &str) -> Option<String> {
    let n = note.trim();
    if n.contains("scope_paths overlap") || n.starts_with("serialize ") {
        // "serialize later after earlier (scope_paths overlap)"
        return Some("为避免改同一处，已改为排队执行".into());
    }
    if n.contains("scope serialize: stop") {
        return Some("多处范围重叠，已尽量改为排队，请再核对步骤顺序".into());
    }
    None
}

/// Unique human soft-accept tips for desk critic_notes.
pub fn soft_accept_human_tips(notes: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for n in notes {
        if let Some(h) = humanize_soft_accept_note(n) {
            if !out.iter().any(|x| x == &h) {
                out.push(h);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::{OnFailure, PlanIR, TaskIR};

    fn task(id: &str, role: Option<TaskRole>, outputs: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: None,
            outputs: outputs.iter().map(|s| (*s).into()).collect(),
            tags: vec![],
            wait_for: vec![],
        }
    }

    #[test]
    fn default_when_multi_no_integrate() {
        let tasks = vec![task("a", None, &[]), task("b", None, &[])];
        let s = merge_check_for_plan(&tasks).unwrap();
        assert!(s.contains("一起") || s.contains("对照"));
        assert!(!s.contains("MERGE.md"));
    }

    #[test]
    fn integrate_without_outputs_no_merge_md() {
        let tasks = vec![
            task("a", Some(TaskRole::Implement), &[]),
            task("i", Some(TaskRole::Integrate), &[]),
        ];
        let s = merge_check_for_plan(&tasks).unwrap();
        assert!(s.contains("整合"));
        assert!(!s.contains("MERGE.md"));
    }

    #[test]
    fn integrate_with_outputs_names_path() {
        let t = task(
            "i",
            Some(TaskRole::Integrate),
            &[".cco-out/join/summary.md"],
        );
        let s = merge_check_for_integrate(&t);
        assert!(s.contains(".cco-out/join/summary.md"));
        assert!(!s.contains("MERGE.md"));
    }

    #[test]
    fn humanize_serialize_note() {
        let h = humanize_soft_accept_note("serialize b after a (scope_paths overlap)").unwrap();
        assert!(h.contains("排队"));
    }

    #[test]
    fn single_task_none() {
        let tasks = vec![task("only", None, &[])];
        assert!(merge_check_for_plan(&tasks).is_none());
        let _ = OnFailure::Pause;
        let _ = PlanIR {
            schema: "x".into(),
            name: "n".into(),
            adapter: "a".into(),
            source_path: std::path::PathBuf::from("p"),
            max_parallel: 1,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "c".into(),
            default_mode: "p".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![],
        };
    }
}
