//! Ensure auto-rework loop (E3) — docs-closeout FAIL → start_rework.
//!
//! [INPUT]: Config · finished run_id
//! [OUTPUT]: Option<ReworkStartResponse> when auto rework starts
//! [POS]: app::run — policy predicates in domain; IO via services facade
//! [PROTOCOL]: **not** a second Mode B entry; P-loop continuation only

use anyhow::Result;

use crate::config::Config;
use crate::domain::inspect::{
    all_blocking_are_docs_closeout, can_start_rework, count_blocking_issues, parse_issues_text,
    parse_verdict_text, InspectVerdict, REWORK_MAX_ROUNDS,
};
use crate::domain::run::is_inspect_gate_error;
use crate::plan::{PlanIR, TaskRole};
use crate::runtime::handoff::{
    self, count_rework_rounds, load_parsed_inspect_issues, read_inspect_verdict, Handoff,
};
use crate::services::{start_rework_from_run, ReworkStartResponse};
use crate::state::{RunState, RunStatus};

/// After a run finishes (reports written), maybe auto-start a docs-closeout rework wave.
///
/// Conditions (all):
/// 1. status Failed/Paused
/// 2. inspect gate error on some failed task **or** VERDICT=FAIL / blocking ISSUES
/// 3. if `auto_rework_docs_only`: all blocking are docs-closeout
/// 4. rework rounds < REWORK_MAX
/// 5. no ACCEPTED_RESIDUAL
/// 6. `auto_rework == true`
pub fn maybe_auto_rework(
    config: &Config,
    run_id: &str,
) -> Result<Option<ReworkStartResponse>> {
    if !config.default.auto_rework {
        return Ok(None);
    }
    let dir = crate::state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let state = RunState::load(&dir)?;
    if !matches!(state.status, RunStatus::Failed | RunStatus::Paused) {
        return Ok(None);
    }

    let plan_path = dir.join("plan.resolved.json");
    if !plan_path.exists() {
        return Ok(None);
    }
    let plan: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?;
    let project = state.project_root.clone();

    let has_gate_err = state.tasks.values().any(|t| {
        is_inspect_gate_error(t.error.as_deref())
            || t.error
                .as_deref()
                .map(|e| e.contains("ISSUES[") || e.contains("REWORK_HOOK"))
                .unwrap_or(false)
    });

    let inspect_task = plan
        .tasks
        .iter()
        .rev()
        .find(|t| t.role == Some(TaskRole::Inspect));
    let verdict = if let Some(t) = inspect_task {
        read_inspect_verdict(t, &project, &project)
    } else {
        let path = project.join(handoff::INSPECT_VERDICT_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_verdict_text(&t))
                .unwrap_or(InspectVerdict::Unknown)
        } else {
            InspectVerdict::Unknown
        }
    };
    let issues = if let Some(t) = inspect_task {
        load_parsed_inspect_issues(t, &project, &project)
    } else {
        let path = project.join(handoff::INSPECT_ISSUES_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_issues_text(&t))
                .unwrap_or_default()
        } else {
            vec![]
        }
    };
    let blocking = count_blocking_issues(&issues);
    if !has_gate_err && verdict != InspectVerdict::Fail && blocking == 0 {
        return Ok(None);
    }

    if config.default.auto_rework_docs_only {
        if blocking == 0 {
            // VERDICT fail without structured blocking — do not auto when docs_only.
            return Ok(None);
        }
        if !all_blocking_are_docs_closeout(&issues) {
            return Ok(None);
        }
    }

    let accepted = Handoff::load(&dir)
        .map(|h| {
            h.open_risks
                .iter()
                .any(|r| r.starts_with("ACCEPTED_RESIDUAL:"))
        })
        .unwrap_or(false);
    let rework_round = count_rework_rounds(&project, &dir);
    let require_inspect = plan.require_inspect
        || plan
            .tasks
            .iter()
            .any(|t| t.role == Some(TaskRole::Inspect));
    let verdict_label = match verdict {
        InspectVerdict::Pass => Some("PASS"),
        InspectVerdict::Fail => Some("FAIL"),
        InspectVerdict::Unknown => Some("UNKNOWN"),
    };
    if !can_start_rework(
        verdict,
        blocking,
        require_inspect,
        accepted,
        rework_round,
        true,
        verdict_label,
    ) {
        return Ok(None);
    }
    if rework_round >= REWORK_MAX_ROUNDS {
        return Ok(None);
    }

    // Timeline marker before start (start_rework also records `rework_wave`).
    // Do **not** use the substring `rework_wave` here — count_rework_rounds
    // would treat this line as a prior wave and bump round to 2.
    if let Ok(mut h) = Handoff::load(&dir) {
        h.timeline.push(format!(
            "{} · ensure_auto · next_round={} · trigger=docs-closeout",
            chrono::Utc::now().to_rfc3339(),
            rework_round + 1
        ));
        let _ = h.save(&dir);
    }

    let resp = start_rework_from_run(config.clone(), run_id)?;
    // Persist auto_rework_run_id for live DTO / UI.
    let marker = dir.join("auto_rework.json");
    let _ = std::fs::write(
        &marker,
        serde_json::to_string_pretty(&serde_json::json!({
            "auto_rework_run_id": resp.run_id,
            "source_run_id": run_id,
            "round": resp.round,
            "ensure_phase": "rework",
            "trigger": "docs-closeout",
        }))
        .unwrap_or_else(|_| "{}".into()),
    );
    Ok(Some(resp))
}

/// Best-effort: never fail the outer finish path.
pub fn maybe_auto_rework_quiet(config: &Config, run_id: &str) -> Option<ReworkStartResponse> {
    match maybe_auto_rework(config, run_id) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(%run_id, error = %e, "ensure auto_rework skipped");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::inspect::{IssueSeverity, ParsedIssue};

    #[test]
    fn docs_only_predicate_uses_classify() {
        let docs = ParsedIssue {
            id: "B6".into(),
            severity: IssueSeverity::Blocking,
            plan_ref: "§9".into(),
            path: "docs/gap.md".into(),
            symptom: "台账未回写".into(),
            fix_wp: "回写".into(),
            raw: "台账".into(),
        };
        assert!(all_blocking_are_docs_closeout(&[docs]));
    }
}
