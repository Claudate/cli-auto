//! Residual evidence gap detection (handwalk / hygiene demote, pure).
//!
//! Split by concern boundary (arch soft ≤400).

use super::super::types::{IssueSeverity, ParsedIssue};
use super::usability::is_usability_blocking_issue;
use super::{hay_hits, issue_haystack};

/// Tokens that mark a mis-graded “evidence gap” as **residual**, not blocking.
///
/// Product rule (P-loop residual): GUI handwalk / 录像 / 未 commit 卫生 / 可选录像
/// must **not** pause the run. Inspect agents often still write severity=blocking
/// or GATE.blocking=1 — host demotes before gate.
const RESIDUAL_HANDWALK_TOKENS: &[&str] = &[
    "handwalk",
    "手点",
    "30s 手",
    "30秒手",
    "30 秒手",
    "真书 30",
    "真书30",
    "录像",
    "录屏",
    "截图",
    "playwright",
    "optional-gui",
    "optional gui",
    "gui 手点",
    "gui手点",
    "无 handwalk",
    "无录像",
    "无截图",
    "手点未做",
    "手点 未做",
];

/// Workspace hygiene often mis-labeled blocking; residual unless business src broken.
const RESIDUAL_HYGIENE_TOKENS: &[&str] = &[
    "uncommitted",
    "未 commit",
    "未commit",
    "未 staged",
    "未staged",
    "gitignore",
    "工作区 m/",
    "m/??",
    "大量工作区",
];

/// True when this ISSUE is residual-class evidence polish, even if agent wrote blocking.
///
/// Used to demote mis-graded handwalk / uncommitted hygiene so inspect does not
/// FAIL-pause a finished implement wave.
///
/// Usability failures are **never** residual evidence gaps.
pub fn is_residual_evidence_gap(issue: &ParsedIssue) -> bool {
    if is_usability_blocking_issue(issue) {
        return false;
    }
    if matches!(
        issue.severity,
        IssueSeverity::Residual | IssueSeverity::OutOfScope
    ) {
        return true;
    }
    // Real map/ledger closeout stays map/blocking (closeout rework path).
    if issue.severity == IssueSeverity::Map {
        return false;
    }
    let hay = issue_haystack(issue);
    let hay_l = hay.to_ascii_lowercase();
    if hay_hits(&hay, RESIDUAL_HANDWALK_TOKENS) {
        return true;
    }
    if hay_hits(&hay, RESIDUAL_HYGIENE_TOKENS) {
        // Hygiene on business src path can still be residual (commit later);
        // only demote when not a hard runtime failure signal.
        let hard = ["panic", "compile", "tsc error", "test fail", "断言失败", "红测"];
        if hard.iter().any(|t| hay_l.contains(t) || hay.contains(t)) {
            return false;
        }
        return true;
    }
    false
}
