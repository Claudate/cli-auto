//! Gap classify for Ensure close-loop (docs-closeout vs evidence).
//!
//! [INPUT]: ParsedIssue rows from ISSUES
//! [OUTPUT]: docs-closeout predicates · GapKind
//! [POS]: domain/inspect — pure; no fs / git
//! [PROTOCOL]: 样本变更须同步单测（含 wros B6/M1）
//! residual/usability demote → [`super::residual`]

use super::types::{IssueSeverity, ParsedIssue};

/// Host-facing gap class for Ensure E1 (first ship: Evidence | MapCloseout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapKind {
    /// Feature / acceptance not met — rework implement.
    Evidence,
    /// Evidence exists; ledger / map / index out of date — closeout or docs rework.
    MapCloseout,
    /// Success criteria weakened — human only (reserved).
    Drift,
    /// Plan too vague to judge — human only (reserved).
    Underspecified,
}

/// Document / ledger path globs (relative). Used by classify + closeout scope.
pub const DOCS_CLOSEOUT_PATH_HINTS: &[&str] = &[
    "docs/",
    "docs\\",
    "readme",
    "claude.md",
    ".cco-out/",
    ".cco-out\\",
    "gap-audit",
    "acceptance",
    "progress/",
    "progress\\",
];

/// Tokens that mark a blocking row as docs/ledger closeout work.
const CLOSEOUT_SYMPTOM_TOKENS: &[&str] = &[
    "docs",
    "closeout",
    "readme",
    "回写",
    "台账",
    "勾选",
    "index",
    "索引",
    "commit",
    "geb",
    "进度",
    "未开工",
    "断链",
    "pointer",
    "map",
    "ledger",
    "验收索引",
    "§",
];

/// True when this ISSUE is a map/ledger/docs closeout item (not business code).
pub fn is_docs_closeout_issue(issue: &ParsedIssue) -> bool {
    if issue.severity == IssueSeverity::Map {
        return true;
    }
    if !issue.severity.is_blocking_for_gate() {
        return false;
    }
    // Business source path → not docs-closeout.
    if path_looks_like_business_src(&issue.path) {
        return false;
    }
    if path_looks_like_docs(&issue.path) {
        return true;
    }
    let hay = format!(
        "{} {} {} {}",
        issue.symptom, issue.fix_wp, issue.plan_ref, issue.raw
    )
    .to_ascii_lowercase();
    // Keep CJK as-is (to_ascii_lowercase leaves them).
    let hay_raw = format!(
        "{} {} {} {}",
        issue.symptom, issue.fix_wp, issue.plan_ref, issue.raw
    );
    let combined = format!("{hay}\n{hay_raw}");
    CLOSEOUT_SYMPTOM_TOKENS.iter().any(|tok| {
        combined
            .to_ascii_lowercase()
            .contains(&tok.to_ascii_lowercase())
            || combined.contains(*tok)
    })
}

/// True when every gate-blocking ISSUE is docs-closeout. Empty → false.
pub fn all_blocking_are_docs_closeout(issues: &[ParsedIssue]) -> bool {
    let blocking: Vec<&ParsedIssue> = issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate())
        .collect();
    if blocking.is_empty() {
        return false;
    }
    blocking.iter().all(|i| is_docs_closeout_issue(i))
}

/// Classify one issue (first ship: MapCloseout vs Evidence).
pub fn classify_kind(issue: &ParsedIssue) -> GapKind {
    if is_docs_closeout_issue(issue) {
        GapKind::MapCloseout
    } else if issue.severity.is_blocking_for_gate() {
        GapKind::Evidence
    } else {
        GapKind::Evidence
    }
}

fn path_looks_like_docs(path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() || p == "n/a" {
        return false;
    }
    let lower = p.to_ascii_lowercase();
    DOCS_CLOSEOUT_PATH_HINTS.iter().any(|h| lower.contains(h))
        || lower.ends_with(".md")
        || lower.contains("readme")
}

fn path_looks_like_business_src(path: &str) -> bool {
    let lower = path.trim().to_ascii_lowercase().replace('\\', "/");
    if lower.is_empty() || lower == "n/a" {
        return false;
    }
    // Explicit business trees (not docs under src).
    let business_markers = [
        "src/",
        "src-tauri/",
        "web/js/",
        "web/css/",
        "crates/",
        "/src/",
        "inkos-rs/",
    ];
    if business_markers.iter().any(|m| lower.contains(m)) {
        // Allow docs-ish under those only if clearly README/md ledger — still business if .rs/.ts
        if lower.ends_with(".rs")
            || lower.ends_with(".ts")
            || lower.ends_with(".js")
            || lower.ends_with(".tsx")
            || lower.ends_with(".jsx")
            || lower.ends_with(".go")
            || lower.ends_with(".py")
        {
            return true;
        }
        // bare src/** without md → business
        if !lower.ends_with(".md") && !lower.contains("readme") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(
        id: &str,
        severity: IssueSeverity,
        path: &str,
        symptom: &str,
        fix_wp: &str,
        raw: &str,
    ) -> ParsedIssue {
        ParsedIssue {
            id: id.into(),
            severity,
            plan_ref: "P0".into(),
            path: path.into(),
            symptom: symptom.into(),
            fix_wp: fix_wp.into(),
            raw: raw.into(),
        }
    }

    /// wros-shaped B6: ledger still 「未开工」 while smoke green.
    #[test]
    fn wros_b6_ledger_is_docs_closeout() {
        let b6 = issue(
            "B6",
            IssueSeverity::Blocking,
            "docs/gap-audit.md",
            "台账 §6/§9/README 仍「未开工」",
            "回写台账勾选与进度句",
            "### B6\n- severity=blocking\n- path: docs/gap-audit.md\n- symptom: 台账 §6/§9/README 仍「未开工」\n- fix_wp: 回写台账勾选",
        );
        assert!(is_docs_closeout_issue(&b6));
        assert_eq!(classify_kind(&b6), GapKind::MapCloseout);
    }

    /// wros-shaped M1: acceptance README pointer broken.
    #[test]
    fn wros_m1_map_is_docs_closeout() {
        let m1 = issue(
            "M1",
            IssueSeverity::Map,
            "docs/acceptance/README.md",
            "acceptance README 断链",
            "修索引指针",
            "### M1\n- severity=map\n- path: docs/acceptance/README.md\n- symptom: acceptance README 断链",
        );
        assert!(is_docs_closeout_issue(&m1));
        assert!(all_blocking_are_docs_closeout(&[m1]));
    }

    #[test]
    fn mixed_business_blocking_not_all_docs() {
        let b6 = issue(
            "B6",
            IssueSeverity::Blocking,
            "docs/gap-audit.md",
            "台账未回写",
            "回写",
            "台账",
        );
        let eng = issue(
            "B1",
            IssueSeverity::Blocking,
            "src/runtime/scheduler/mod.rs",
            "引擎未实现 failover 分支",
            "实现 scheduler failover",
            "引擎未实现",
        );
        assert!(is_docs_closeout_issue(&b6));
        assert!(!is_docs_closeout_issue(&eng));
        assert_eq!(classify_kind(&eng), GapKind::Evidence);
        assert!(!all_blocking_are_docs_closeout(&[b6, eng]));
    }

    #[test]
    fn empty_blocking_is_not_all_docs() {
        assert!(!all_blocking_are_docs_closeout(&[]));
        let residual = issue(
            "R1",
            IssueSeverity::Residual,
            "docs/x.md",
            "未 commit",
            "可选",
            "residual",
        );
        assert!(!all_blocking_are_docs_closeout(&[residual]));
    }

    #[test]
    fn map_severity_always_docs() {
        let m = issue(
            "M2",
            IssueSeverity::Map,
            "n/a",
            "GEB 指针滞后",
            "更新 CLAUDE.md",
            "map",
        );
        assert!(is_docs_closeout_issue(&m));
    }
}
