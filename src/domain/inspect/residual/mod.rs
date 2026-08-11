//! Residual / usability grade normalize for host gate (P-loop).
//!
//! Split by pure-function boundary (arch soft ≤400):
//! - [`usability`] — usability / intent-degradation promote
//! - [`evidence`] — handwalk / hygiene demote
//! - [`tests`] — residual unit tests
//!
//! [INPUT]: ParsedIssue rows from ISSUES
//! [OUTPUT]: demote handwalk · promote real usability · effective blocking counts
//! [POS]: domain/inspect — pure; no fs / git
//! [PROTOCOL]: 否定句不得升 blocking；意图静默降级升 blocking；样本变更同步单测

mod evidence;
mod usability;

pub use evidence::is_residual_evidence_gap;
pub use usability::is_usability_blocking_issue;

use super::types::{IssueSeverity, ParsedIssue};

pub(crate) fn issue_haystack(issue: &ParsedIssue) -> String {
    format!(
        "{} {} {} {} {}",
        issue.id, issue.symptom, issue.fix_wp, issue.plan_ref, issue.raw
    )
}

pub(crate) fn hay_hits(hay: &str, tokens: &[&str]) -> bool {
    let hay_l = hay.to_ascii_lowercase();
    tokens.iter().any(|tok| {
        let t = tok.to_ascii_lowercase();
        hay_l.contains(&t) || hay.contains(*tok)
    })
}

/// Normalize ISSUES severities for host gate:
/// 1. promote usability / anti-common-sense residual → blocking
/// 2. demote mis-graded handwalk/hygiene blocking → residual
pub fn demote_residual_evidence_issues(issues: &mut [ParsedIssue]) {
    for i in issues.iter_mut() {
        if is_usability_blocking_issue(i)
            && matches!(
                i.severity,
                IssueSeverity::Residual | IssueSeverity::Blocking | IssueSeverity::Map
            )
        {
            // Map stays map (closeout path). Residual usability → blocking.
            if i.severity == IssueSeverity::Residual {
                i.severity = IssueSeverity::Blocking;
            }
            continue;
        }
        if i.severity.is_blocking_for_gate() && is_residual_evidence_gap(i) {
            i.severity = IssueSeverity::Residual;
        }
    }
}

/// Gate-blocking count **after** residual-evidence demotion / usability promote.
pub fn effective_blocking_count(issues: &[ParsedIssue]) -> usize {
    issues
        .iter()
        .filter(|i| {
            if is_usability_blocking_issue(i) {
                return i.severity != IssueSeverity::OutOfScope;
            }
            i.severity.is_blocking_for_gate() && !is_residual_evidence_gap(i)
        })
        .count()
}

/// Host gate: PASS when no **real** blocking remains after demotion.
///
/// Returns `(blocked, effective_blocking_n)`. Prefer this over raw GATE.blocking
/// when ISSUES body proves all open rows are residual evidence gaps.
pub fn gate_counts_after_residual_demote(
    issues: &[ParsedIssue],
    gate_blocking: Option<usize>,
    gate_result_fail: bool,
) -> (bool, usize) {
    let n = effective_blocking_count(issues);
    if n == 0 {
        // Residual-only (or empty ISSUES with agent FAIL/GATE fail about handwalk):
        // do not pause. Empty ISSUES + GATE fail still blocks (unknown real gap).
        if !issues.is_empty() || !gate_result_fail {
            return (false, 0);
        }
        // GATE fail + no ISSUES body → keep fail-closed if gate said blocking.
        let g = gate_blocking.unwrap_or(0);
        return (
            g > 0 || gate_result_fail,
            g.max(if gate_result_fail { 1 } else { 0 }),
        );
    }
    (true, n)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
