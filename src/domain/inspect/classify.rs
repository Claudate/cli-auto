//! Gap classify for Ensure close-loop (docs-closeout vs evidence).
//!
//! [INPUT]: ParsedIssue rows from ISSUES
//! [OUTPUT]: docs-closeout predicates · GapKind
//! [POS]: domain/inspect — pure; no fs / git
//! [PROTOCOL]: 样本变更须同步单测（含 wros B6/M1）

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
    CLOSEOUT_SYMPTOM_TOKENS
        .iter()
        .any(|tok| combined.to_ascii_lowercase().contains(&tok.to_ascii_lowercase()) || combined.contains(tok))
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

/// Tokens that mark a mis-graded “evidence gap” as **residual**, not blocking.
///
/// Product rule (P-loop residual): GUI handwalk / 录像 / 未 commit 卫生 / 可选录像
/// must **not** pause the run. Inspect agents often still write severity=blocking
/// or GATE.blocking=1 — host demotes before gate.
/// Handwalk / GUI recording — residual polish, never host-blocking alone.
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
pub fn is_residual_evidence_gap(issue: &ParsedIssue) -> bool {
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
    let hay = format!(
        "{} {} {} {} {}",
        issue.id, issue.symptom, issue.fix_wp, issue.plan_ref, issue.raw
    );
    let hay_l = hay.to_ascii_lowercase();
    let hit = |tokens: &[&str]| {
        tokens.iter().any(|tok| {
            let t = tok.to_ascii_lowercase();
            hay_l.contains(&t) || hay.contains(*tok)
        })
    };
    if hit(RESIDUAL_HANDWALK_TOKENS) {
        return true;
    }
    if hit(RESIDUAL_HYGIENE_TOKENS) {
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

/// Demote mis-graded residual evidence rows in place (host SoT for gate counts).
pub fn demote_residual_evidence_issues(issues: &mut [ParsedIssue]) {
    for i in issues.iter_mut() {
        if i.severity.is_blocking_for_gate() && is_residual_evidence_gap(i) {
            i.severity = IssueSeverity::Residual;
        }
    }
}

/// Gate-blocking count **after** residual-evidence demotion.
pub fn effective_blocking_count(issues: &[ParsedIssue]) -> usize {
    issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate() && !is_residual_evidence_gap(i))
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
        return (g > 0 || gate_result_fail, g.max(if gate_result_fail { 1 } else { 0 }));
    }
    (true, n)
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
    DOCS_CLOSEOUT_PATH_HINTS
        .iter()
        .any(|h| lower.contains(h))
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

    /// wros handwalk: agent marks blocking for missing 30s GUI log — host demotes.
    #[test]
    fn handwalk_blocking_demoted_to_residual() {
        let b1 = issue(
            "B1",
            IssueSeverity::Blocking,
            "docs/one/logs/**",
            "真书 30 秒主路径手点观察未写入可验收 logs",
            "optional-gui-handwalk-record",
            "### B1\n- severity=blocking\n- symptom: 真书 30 秒手点未做\n- fix_wp: optional-gui-handwalk-record",
        );
        assert!(is_residual_evidence_gap(&b1));
        let mut rows = vec![b1];
        demote_residual_evidence_issues(&mut rows);
        assert_eq!(rows[0].severity, IssueSeverity::Residual);
        assert_eq!(effective_blocking_count(&rows), 0);
        let (blocked, n) = gate_counts_after_residual_demote(&rows, Some(1), true);
        assert!(!blocked && n == 0);
    }

    #[test]
    fn real_feature_blocking_not_demoted() {
        let eng = issue(
            "B1",
            IssueSeverity::Blocking,
            "src/runtime/scheduler/mod.rs",
            "引擎未实现 failover 分支",
            "实现 scheduler failover",
            "severity=blocking 引擎未实现",
        );
        assert!(!is_residual_evidence_gap(&eng));
        assert_eq!(effective_blocking_count(&[eng]), 1);
    }
}
