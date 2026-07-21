//! Pure inspect gate rules (A1-5).
//!
//! [INPUT]: TaskIR role/outputs · InspectVerdict · ParsedIssue slices
//! [OUTPUT]: gate applicability · fail reasons · push decision · rework notes
//! [POS]: domain/inspect — scheduler / adapter call these; no fs
//! [PROTOCOL]: 变更时同步 scheduler gates 委托路径；禁止正文解析落在 scheduler

use crate::domain::plan::{TaskIR, TaskRole};

use super::types::{
    InspectVerdict, IssueSeverity, ParsedIssue, INSPECT_ISSUES_REL, INSPECT_VERDICT_REL,
    REWORK_MAX_ROUNDS,
};

pub fn looks_like_verdict_path(rel: &str) -> bool {
    rel.to_ascii_lowercase().contains("verdict")
}

pub fn looks_like_issues_path(rel: &str) -> bool {
    rel.to_ascii_lowercase().contains("issues")
}

/// Candidate VERDICT paths: declared outputs that look like VERDICT, plus convention for role=inspect.
pub fn verdict_candidate_paths(task: &TaskIR) -> Vec<String> {
    let mut out: Vec<String> = task
        .outputs
        .iter()
        .filter(|o| looks_like_verdict_path(o))
        .cloned()
        .collect();
    // role=inspect always checks conventional path even if not listed in outputs.
    if task.role == Some(TaskRole::Inspect) && !out.iter().any(|o| o == INSPECT_VERDICT_REL) {
        out.push(INSPECT_VERDICT_REL.into());
    }
    out
}

/// Candidate ISSUES paths for rework consumption.
pub fn issues_candidate_paths(task: &TaskIR) -> Vec<String> {
    let mut out: Vec<String> = task
        .outputs
        .iter()
        .filter(|o| looks_like_issues_path(o))
        .cloned()
        .collect();
    if task.role == Some(TaskRole::Inspect) && !out.iter().any(|o| o == INSPECT_ISSUES_REL) {
        out.push(INSPECT_ISSUES_REL.into());
    }
    // Fallback: if VERDICT convention was used, also try ISSUES convention.
    if out.is_empty()
        && task
            .outputs
            .iter()
            .any(|o| looks_like_verdict_path(o) || o == INSPECT_VERDICT_REL)
    {
        out.push(INSPECT_ISSUES_REL.into());
    }
    out
}

/// Whether this task should run VERDICT gate after Done (role=inspect or declared VERDICT output).
pub fn task_has_verdict_gate(task: &TaskIR) -> bool {
    task.role == Some(TaskRole::Inspect)
        || task.outputs.iter().any(|o| looks_like_verdict_path(o))
}

/// Count ISSUES that block plan-loop success (blocking + map).
pub fn count_blocking_issues(issues: &[ParsedIssue]) -> usize {
    issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate())
        .count()
}

/// True when PASS is invalid because blocking/map ISSUES remain (P-loop R-inspect).
pub fn inspect_pass_blocked(issues: &[ParsedIssue]) -> (bool, usize) {
    let n = count_blocking_issues(issues);
    (n > 0, n)
}

/// Lightweight rework-hook note (P2-3 + P-loop): ledger breadcrumb with fix_wp hints.
pub fn rework_placeholder_note(task_id: &str, issues: &[String]) -> String {
    if issues.is_empty() {
        format!(
            "REWORK_HOOK: inspect task `{task_id}` VERDICT=FAIL; no ISSUES body — open `.cco-out/inspect/` and start a rework wave (desktop「回补并再巡检」 or services::start_rework_from_run)"
        )
    } else {
        format!(
            "REWORK_HOOK: inspect task `{task_id}` — {} ISSUE line(s); generate rework TaskIR via start_rework_from_run (max {REWORK_MAX_ROUNDS} rounds); host does not auto-merge/PR",
            issues.len()
        )
    }
}

/// Post-Done inspect gate fail reason (pure). `None` = keep Done.
///
/// Scheduler/adapters must not re-implement this match; only supply already-parsed
/// verdict + issue counts (text parse stays in domain::inspect::parse).
pub fn inspect_gate_fail_reason(
    verdict: InspectVerdict,
    blocking_n: usize,
    issues_len: usize,
    treat_unknown_as_fail: bool,
    task_id: &str,
) -> Option<String> {
    let blocked = blocking_n > 0;
    match verdict {
        InspectVerdict::Fail => {
            let issues_hint = if issues_len == 0 {
                format!("see {INSPECT_ISSUES_REL}")
            } else {
                format!(
                    "{issues_len} ISSUES line(s) for rework (Open risks ISSUES[{task_id}])"
                )
            };
            Some(format!("inspect VERDICT=FAIL ({issues_hint})"))
        }
        InspectVerdict::Unknown if treat_unknown_as_fail => Some(format!(
            "inspect VERDICT=UNKNOWN (require_inspect/role=inspect treats Unknown as FAIL; expected {INSPECT_VERDICT_REL})"
        )),
        InspectVerdict::Pass if blocked => Some(format!(
            "inspect VERDICT=PASS but {blocking_n} blocking/map ISSUE(s) remain — cannot close plan loop (P-loop R-inspect)"
        )),
        _ => None,
    }
}

/// Pure push-after-inspect decision (sys-post-git-push). `Ok(())` = may start.
pub fn push_inspect_gate_decision(
    has_inspect_task: bool,
    verdict: InspectVerdict,
    blocked: bool,
    blocking_n: usize,
) -> Result<(), String> {
    if !has_inspect_task {
        return Err(
            "CCO_PUSH_SKIPPED reason=no_inspect_task (host: push requires inspect before commit)"
                .into(),
        );
    }
    match verdict {
        InspectVerdict::Pass if !blocked => Ok(()),
        InspectVerdict::Pass => Err(format!(
            "CCO_PUSH_SKIPPED reason=inspect_blocking_issues n={blocking_n} (host: VERDICT=PASS but blocking ISSUES remain)"
        )),
        InspectVerdict::Fail => Err(
            "CCO_PUSH_SKIPPED reason=inspect_not_pass (host: VERDICT=FAIL — no commit/push)".into(),
        ),
        InspectVerdict::Unknown => Err(format!(
            "CCO_PUSH_SKIPPED reason=inspect_unknown (host: missing or unreadable {INSPECT_VERDICT_REL}; no commit/push)"
        )),
    }
}

/// Whether residual-class issues only (for UI counts).
pub fn count_residual_issues(issues: &[ParsedIssue]) -> usize {
    issues
        .iter()
        .filter(|i| {
            matches!(
                i.severity,
                IssueSeverity::Residual | IssueSeverity::OutOfScope
            )
        })
        .count()
}

/// Pure can_rework decision (adapter supplies rework_round + terminal run flag).
pub fn can_start_rework(
    verdict: InspectVerdict,
    blocking_count: usize,
    require_inspect: bool,
    accepted_residual: bool,
    rework_round: u32,
    run_is_terminal_for_rework: bool,
    verdict_label: Option<&str>,
) -> bool {
    let needs_rework = matches!(verdict, InspectVerdict::Fail)
        || blocking_count > 0
        || (verdict_label == Some("UNKNOWN") && require_inspect);
    needs_rework
        && !accepted_residual
        && rework_round < REWORK_MAX_ROUNDS
        && run_is_terminal_for_rework
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::TaskIR;

    fn sample_task(role: Option<TaskRole>, outputs: Vec<String>) -> TaskIR {
        TaskIR {
            id: "t".into(),
            title: "t".into(),
            depends_on: vec![],
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: "p".into(),
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: None,
            outputs,
            tags: vec![],
        }
    }

    #[test]
    fn task_has_verdict_gate_by_role_or_output() {
        assert!(task_has_verdict_gate(&sample_task(
            Some(TaskRole::Inspect),
            vec![]
        )));
        assert!(task_has_verdict_gate(&sample_task(
            None,
            vec![".cco-out/inspect/VERDICT.md".into()]
        )));
        assert!(!task_has_verdict_gate(&sample_task(
            None,
            vec!["out.txt".into()]
        )));
    }

    #[test]
    fn inspect_gate_fail_reason_covers_fail_unknown_pass_blocked() {
        assert!(inspect_gate_fail_reason(InspectVerdict::Fail, 0, 0, false, "i").is_some());
        assert!(inspect_gate_fail_reason(InspectVerdict::Unknown, 0, 0, true, "i").is_some());
        assert!(inspect_gate_fail_reason(InspectVerdict::Unknown, 0, 0, false, "i").is_none());
        assert!(inspect_gate_fail_reason(InspectVerdict::Pass, 2, 2, false, "i").is_some());
        assert!(inspect_gate_fail_reason(InspectVerdict::Pass, 0, 0, false, "i").is_none());
    }

    #[test]
    fn push_gate_requires_pass_without_blocking() {
        assert!(push_inspect_gate_decision(false, InspectVerdict::Pass, false, 0).is_err());
        assert!(push_inspect_gate_decision(true, InspectVerdict::Pass, false, 0).is_ok());
        assert!(push_inspect_gate_decision(true, InspectVerdict::Fail, false, 0).is_err());
        assert!(push_inspect_gate_decision(true, InspectVerdict::Pass, true, 1).is_err());
    }
}
