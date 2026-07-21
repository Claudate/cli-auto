//! Inspect product IO wrappers (A1-5 adapter).
//!
//! Pure parse/gate rules live in `domain::inspect`. This module only reads files
//! and delegates to pure functions — **scheduler must not re-parse VERDICT text**.
//!
//! [INPUT]: TaskIR · work_dir · project_root · PlanIR
//! [OUTPUT]: InspectVerdict · ParsedIssue · gate Result
//! [POS]: runtime/handoff
//! [PROTOCOL]: 禁止在 scheduler/* 复制 parse_verdict_text

use std::path::Path;

use crate::domain::inspect::{
    inspect_pass_blocked, parse_issues_text, parse_verdict_text, push_inspect_gate_decision,
    InspectVerdict, ParsedIssue, INSPECT_ISSUES_REL,
};
use crate::plan::{PlanIR, TaskIR, TaskRole};

use super::paths::resolve_output_path;

// Pure helpers used by this IO layer (call sites re-export via mod.rs).
pub use crate::domain::inspect::{
    issues_candidate_paths, task_has_verdict_gate, verdict_candidate_paths,
};

/// Read inspect VERDICT product; Unknown if no file / unparseable.
pub fn read_inspect_verdict(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> InspectVerdict {
    let candidates = verdict_candidate_paths(task);
    if candidates.is_empty() {
        return InspectVerdict::Unknown;
    }
    for rel in candidates {
        let path = resolve_output_path(&rel, work_dir, project_root);
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let v = parse_verdict_text(&text);
        if v != InspectVerdict::Unknown {
            return v;
        }
    }
    InspectVerdict::Unknown
}

/// Read raw ISSUES.md text (first existing candidate).
pub fn read_inspect_issues_text(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Option<String> {
    for rel in issues_candidate_paths(task) {
        let path = resolve_output_path(&rel, work_dir, project_root);
        if !path.is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some(text);
        }
    }
    None
}

/// Read + parse ISSUES; empty if no file / none.
pub fn load_parsed_inspect_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Vec<ParsedIssue> {
    match read_inspect_issues_text(task, work_dir, project_root) {
        Some(text) => parse_issues_text(&text),
        None => vec![],
    }
}

/// True when PASS is invalid because blocking/map ISSUES remain (P-loop R-inspect).
pub fn inspect_pass_blocked_by_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> (bool, usize) {
    let parsed = load_parsed_inspect_issues(task, work_dir, project_root);
    inspect_pass_blocked(&parsed)
}

/// Read ISSUES product into short consumable lines (for Open risks / rework hook).
/// Stable format: each risk line is `ISSUES[<task_id>]: severity=… <snippet>`.
pub fn collect_inspect_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Vec<String> {
    let parsed = load_parsed_inspect_issues(task, work_dir, project_root);
    if !parsed.is_empty() {
        return parsed
            .into_iter()
            .take(12)
            .map(|i| {
                let snippet: String = i
                    .raw
                    .lines()
                    .next()
                    .unwrap_or(&i.symptom)
                    .chars()
                    .take(180)
                    .collect();
                format!(
                    "ISSUES[{}]: severity={} plan_ref={} {}",
                    task.id,
                    i.severity.as_str(),
                    if i.plan_ref.is_empty() {
                        "n/a"
                    } else {
                        &i.plan_ref
                    },
                    snippet
                )
            })
            .collect();
    }
    // Fallback: raw lines when parse yielded nothing but file exists.
    let mut lines = Vec::new();
    if let Some(text) = read_inspect_issues_text(task, work_dir, project_root) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return lines;
        }
        let mut n = 0usize;
        for line in trimmed.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            if lower == "无" || lower == "none" || lower == "n/a" || lower == "na" {
                continue;
            }
            let snippet: String = t.chars().take(200).collect();
            lines.push(format!("ISSUES[{}]: {snippet}", task.id));
            n += 1;
            if n >= 12 {
                break;
            }
        }
        if lines.is_empty() {
            lines.push(format!(
                "ISSUES[{}]: (file present, no actionable items) {}",
                task.id, INSPECT_ISSUES_REL
            ));
        }
    }
    lines
}

/// Host hard-gate for `sys-post-git-push`: only allow when inspect VERDICT is PASS
/// and no blocking/map ISSUES remain.
///
/// Returns `Ok(())` if push may start; `Err(reason)` if it must be skipped (never spawn).
/// If the plan has no inspect dependency for this push task, returns Ok (legacy / no gate).
pub fn system_push_inspect_gate(
    plan: &PlanIR,
    push: &TaskIR,
    project_root: &Path,
) -> Result<(), String> {
    use crate::plan::{is_system_post_task, SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID};

    if push.id != SYS_POST_GIT_PUSH_ID && !is_system_post_task(&push.id) {
        return Ok(());
    }
    // Only gate the git-push system task (not inspect itself).
    if push.id != SYS_POST_GIT_PUSH_ID {
        return Ok(());
    }
    // Require inspect in plan and as dependency (or present as role=inspect).
    let inspect_task = plan.task(SYS_POST_INSPECT_ID).or_else(|| {
        plan.tasks
            .iter()
            .find(|t| t.role == Some(TaskRole::Inspect))
    });
    let Some(inspect) = inspect_task else {
        return push_inspect_gate_decision(false, InspectVerdict::Unknown, false, 0);
    };
    // Prefer project_root for system inspect products (no worktree for inspect by default).
    let wd = project_root;
    let verdict = read_inspect_verdict(inspect, wd, project_root);
    let (blocked, blocking_n) = inspect_pass_blocked_by_issues(inspect, wd, project_root);
    push_inspect_gate_decision(true, verdict, blocked, blocking_n)
}

