//! Board lifecycle: shell · task start/end · run end (A1-5 adapter).
//!
//! [INPUT]: PlanIR · RunState · TaskResult
//! [OUTPUT]: handoff.md/json updates
//! [POS]: runtime/handoff — HandoffStore free-function surface
//! [PROTOCOL]: on_task_end 仍可调 domain 纯规则 + inspect_io；scheduler 只调本 API

use std::path::Path;

use anyhow::Result;
use chrono::Utc;

use crate::domain::inspect::{
    rework_placeholder_note, task_has_verdict_gate, InspectVerdict, INSPECT_ISSUES_REL,
};
use crate::plan::{PlanIR, TaskIR};
use crate::runtime::provider::{TaskResult, TaskStatus};
use crate::state::{RunState, RunStatus};

use super::inspect_io::{collect_inspect_issues, read_inspect_verdict};
use super::model::{default_next_instructions, status_label, Fragment, Handoff};
use super::paths::{resolve_output_path, write_task_diff};

pub(super) fn load_or_init(plan: &PlanIR, state: &RunState) -> Result<Handoff> {
    let path = Handoff::path_json(&state.run_dir);
    if path.exists() {
        Handoff::load(&state.run_dir)
    } else {
        Ok(Handoff::init_shell(plan, state))
    }
}

/// Create empty handoff shell and write to disk.
pub fn write_shell(plan: &PlanIR, state: &RunState) -> Result<()> {
    let h = Handoff::init_shell(plan, state);
    h.save(&state.run_dir)
}

/// Board → running on task start.
pub fn on_task_start(plan: &PlanIR, state: &RunState, task_id: &str) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();
    h.status = "running".into();
    h.set_board_status(task_id, "running", None, "");
    h.push_timeline(format!(
        "{} · task_start · {task_id}",
        Utc::now().to_rfc3339()
    ));
    let done: Vec<String> = h
        .board
        .iter()
        .filter(|r| r.status == "done" || r.status == "skipped")
        .map(|r| r.id.clone())
        .collect();
    h.instructions_for_next = default_next_instructions(plan, &done);
    h.save(&state.run_dir)
}

/// Merge fragment after task terminal; update Board / Timeline / Open risks.
pub fn on_task_end(
    plan: &PlanIR,
    state: &RunState,
    task: &TaskIR,
    result: &TaskResult,
    work_dir: Option<&Path>,
) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();

    let st_label = status_label(result.status);
    let cost = result.cost_usd;
    let notes = result
        .error
        .as_deref()
        .map(|e| e.chars().take(120).collect::<String>())
        .unwrap_or_default();

    h.set_board_status(&task.id, st_label, cost, &notes);
    h.push_timeline(format!(
        "{} · task_end · {} · {st_label}",
        Utc::now().to_rfc3339(),
        task.id
    ));

    let wd = work_dir
        .map(|p| p.to_path_buf())
        .or_else(|| state.tasks.get(&task.id).and_then(|t| t.work_dir.clone()))
        .unwrap_or_else(|| state.project_root.clone());

    let branch = state
        .tasks
        .get(&task.id)
        .and_then(|t| t.worktree_branch.clone());

    let mut artifacts = Vec::new();
    for o in &task.outputs {
        let path = resolve_output_path(o, &wd, &state.project_root);
        if path.exists() {
            artifacts.push(o.clone());
        }
    }

    // P2-2: host-generated per-task diff list for inspect consumption.
    if let Ok(Some(rel)) = write_task_diff(task, &wd, &state.project_root) {
        if !artifacts.iter().any(|a| a == &rel) {
            artifacts.push(rel);
        }
    }

    let summary = extract_summary(task, &wd, &state.project_root, result);

    let mut risks = Vec::new();
    if result.status != TaskStatus::Done {
        if let Some(err) = &result.error {
            risks.push(format!("{}: {err}", task.id));
        } else {
            risks.push(format!("{} ended as {st_label}", task.id));
        }
    }

    // P2-3: on VERDICT=FAIL (or error mentions it), fold ISSUES into fragment risks + Open risks.
    let verdict_fail = result
        .error
        .as_deref()
        .map(|e| e.contains("VERDICT=FAIL") || e.contains("inspect VERDICT"))
        .unwrap_or(false)
        || (task_has_verdict_gate(task)
            && read_inspect_verdict(task, &wd, &state.project_root) == InspectVerdict::Fail);
    let mut rework_note: Option<String> = None;
    if verdict_fail {
        let issues = collect_inspect_issues(task, &wd, &state.project_root);
        if issues.is_empty() {
            // Still leave a stable ISSUES clue even if file missing.
            risks.push(format!(
                "ISSUES[{}]: VERDICT=FAIL — see {} (missing or empty)",
                task.id, INSPECT_ISSUES_REL
            ));
        } else {
            for line in &issues {
                if !risks.iter().any(|r| r == line) {
                    risks.push(line.clone());
                }
            }
        }
        let note = rework_placeholder_note(&task.id, &issues);
        risks.push(note.clone());
        rework_note = Some(note);
        h.push_timeline(format!(
            "{} · inspect_verdict_fail · {} · ISSUES folded",
            Utc::now().to_rfc3339(),
            task.id
        ));
    }

    h.fragments.insert(
        task.id.clone(),
        Fragment {
            status: st_label.into(),
            provider: task.provider.clone(),
            work_dir: Some(wd.display().to_string()),
            branch,
            summary,
            artifacts,
            risks: risks.clone(),
        },
    );

    // Rebuild open risks from all fragments + current
    h.open_risks = h
        .fragments
        .values()
        .flat_map(|f| f.risks.iter().cloned())
        .collect();

    let done: Vec<String> = h
        .board
        .iter()
        .filter(|r| r.status == "done" || r.status == "skipped")
        .map(|r| r.id.clone())
        .collect();
    let mut next = default_next_instructions(plan, &done);
    if let Some(note) = rework_note {
        // Stable rework hook surface for humans / next wave (not auto-scheduled).
        next = format!(
            "- {note}\n- consumable ISSUES lines are under Open risks (prefix ISSUES[{}])\n{next}",
            task.id
        );
    }
    h.instructions_for_next = next;

    h.save(&state.run_dir)
}

/// Final run status stamp on handoff.
pub fn on_run_end(plan: &PlanIR, state: &RunState, status: RunStatus) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();
    h.status = match status {
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Paused => "paused",
        RunStatus::Aborted => "aborted",
        RunStatus::Running => "running",
        RunStatus::Validated => "validated",
        RunStatus::Init => "init",
    }
    .into();
    h.push_timeline(format!(
        "{} · run_end · {}",
        Utc::now().to_rfc3339(),
        h.status
    ));
    h.save(&state.run_dir)
}

fn extract_summary(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
    result: &TaskResult,
) -> String {
    // Prefer declared outputs that look like summary / md
    for o in &task.outputs {
        let lower = o.to_ascii_lowercase();
        if lower.contains("summary") || lower.ends_with(".md") {
            let path = resolve_output_path(o, work_dir, project_root);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let s: String = text.chars().take(400).collect();
                if !s.trim().is_empty() {
                    return s.trim().replace('\n', " ");
                }
            }
        }
    }
    // Fallback: result.raw.result string
    if let Some(s) = result
        .raw
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(200).collect::<String>())
    {
        if !s.is_empty() {
            return s;
        }
    }
    if let Some(err) = &result.error {
        return err.chars().take(200).collect();
    }
    String::new()
}
