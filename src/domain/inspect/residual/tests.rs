//! Residual unit tests (split from residual.rs for soft ≤400).

use super::*;
use super::super::types::{IssueSeverity, ParsedIssue};

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

/// Anti-common-sense / unusable main path must not residual-PASS.
#[test]
fn usability_residual_promoted_to_blocking() {
    let r1 = issue(
        "R1",
        IssueSeverity::Residual,
        "js/model/plant.js",
        "新建即已完成：添加植物默认 lastWatered=今天，反常识",
        "改默认或表单必填上次浇水日",
        "### R1\nseverity: residual\nsymptom: 新建即已完成 反常识",
    );
    assert!(is_usability_blocking_issue(&r1));
    assert!(!is_residual_evidence_gap(&r1));
    let mut rows = vec![r1];
    demote_residual_evidence_issues(&mut rows);
    assert_eq!(rows[0].severity, IssueSeverity::Blocking);
    assert_eq!(effective_blocking_count(&rows), 1);
    let (blocked, n) = gate_counts_after_residual_demote(&rows, Some(0), false);
    assert!(blocked && n == 1);
}

#[test]
fn isolation_failure_counts_even_if_labeled_residual() {
    let r = issue(
        "R2",
        IssueSeverity::Residual,
        "js/app.js",
        "一点改多对象：点 A 已浇后 B 的 lastWatered 也被改",
        "按 id 更新单盆",
        "severity=residual 操作串对象",
    );
    assert!(is_usability_blocking_issue(&r));
    assert_eq!(effective_blocking_count(&[r]), 1);
}

#[test]
fn handwalk_still_demotes_when_not_usability() {
    let b1 = issue(
        "B1",
        IssueSeverity::Blocking,
        "docs/one/logs/**",
        "真书 30 秒主路径手点观察未写入",
        "optional-gui-handwalk-record",
        "severity=blocking 手点未做",
    );
    assert!(is_residual_evidence_gap(&b1));
    assert!(!is_usability_blocking_issue(&b1));
}

/// Regression (test/9 t5 R3): residual scaffold CSS with *negated* usability
/// wording must stay residual — host must not promote → auto rework.
#[test]
fn negated_wrong_state_residual_not_promoted() {
    let r3 = issue(
        "R3",
        IssueSeverity::Residual,
        "src/styles/layout.css (`.shell-placeholder*`)",
        "Scaffold leftover placeholder styles remain in layout CSS after t3 filled real pages. Unused; does not affect main path or wrong state.",
        "Grep shell-placeholder — defined in CSS, not referenced by current page components.",
        "### R3\nseverity: residual\npath: src/styles/layout.css (`.shell-placeholder*`)\nphenomenon: Scaffold leftover placeholder styles remain in layout CSS after t3 filled real pages. Unused; does not affect main path or wrong state.\nrepro: Grep `shell-placeholder` — defined in CSS, not referenced by current page components.",
    );
    assert!(
        !is_usability_blocking_issue(&r3),
        "negated 'wrong state' must not count as usability failure"
    );
    assert!(is_residual_evidence_gap(&r3));
    let mut rows = vec![r3];
    demote_residual_evidence_issues(&mut rows);
    assert_eq!(rows[0].severity, IssueSeverity::Residual);
    assert_eq!(effective_blocking_count(&rows), 0);
    let (blocked, n) = gate_counts_after_residual_demote(&rows, Some(0), false);
    assert!(!blocked && n == 0);
}

#[test]
fn affirmed_wrong_state_still_blocks() {
    let b = issue(
        "B1",
        IssueSeverity::Residual,
        "src/cart/store.ts",
        "remove line leaves wrong state in subtotal",
        "recompute subtotal from lines only",
        "severity: residual\nsymptom: cart shows wrong state after remove",
    );
    assert!(is_usability_blocking_issue(&b));
    let mut rows = vec![b];
    demote_residual_evidence_issues(&mut rows);
    assert_eq!(rows[0].severity, IssueSeverity::Blocking);
    assert_eq!(effective_blocking_count(&rows), 1);
}

/// Regression (test/9 R1): plan asked for real-feel product photos; agent filed
/// “illustrative SVG / not photo stock” as residual PASS — host must promote.
#[test]
fn intent_degrade_illustrative_svg_promotes_to_blocking() {
    let r1 = issue(
        "R1",
        IssueSeverity::Residual,
        "public/images/products/**, public/images/hero/**",
        "Product and hero art are on-disk illustrative SVGs (not photo stock). Still local, non-placeholder, and alt-bearing per plan.",
        "Optional later photo-stock swap; not required for V1 gate.",
        "### R1\nseverity: residual\npath: public/images/**\nphenomenon: illustrative SVGs (not photo stock)\nfix_wp: Optional later photo-stock swap",
    );
    assert!(
        is_usability_blocking_issue(&r1),
        "illustrative SVG / not photo stock is plan-intent degradation"
    );
    let mut rows = vec![r1];
    demote_residual_evidence_issues(&mut rows);
    assert_eq!(rows[0].severity, IssueSeverity::Blocking);
    assert_eq!(effective_blocking_count(&rows), 1);
}

#[test]
fn t5_full_issues_body_r1_intent_blocks_r3_stays_residual() {
    use crate::domain::inspect::parse_issues_text;
    let text = r#"# Inspect ISSUES · t5

No blocking or map issues. Residual notes only (do not force FAIL).

### R1
severity: residual
path: public/images/products/**, public/images/hero/**
phenomenon: Product and hero art are on-disk illustrative SVGs (not photo stock). Still local, non-placeholder, and alt-bearing per plan; no placehold.co / via.placeholder in source or production bundle.
repro: Open any product card or hero; inspect `src` → `/images/...svg`.

### R2
severity: residual
path: tests/** (absent)
phenomenon: No automated unit/e2e suite under `tests/**`. Acceptance relies on `npm run build`, preview HTTP smoke, and source-level main-path review.
repro: `ls tests 2>/dev/null` → missing; build + preview smoke still green.

### R3
severity: residual
path: src/styles/layout.css (`.shell-placeholder*`)
phenomenon: Scaffold leftover placeholder styles remain in layout CSS after t3 filled real pages. Unused; does not affect main path or wrong state.
repro: Grep `shell-placeholder` — defined in CSS, not referenced by current page components.
"#;
    let mut parsed = parse_issues_text(text);
    assert!(
        parsed.iter().any(|i| i.id == "R3"),
        "parser must keep R3 id; got {:?}",
        parsed.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
    demote_residual_evidence_issues(&mut parsed);
    // R1 intent degradation → blocking; R2 hygiene + R3 dead CSS stay residual.
    assert_eq!(effective_blocking_count(&parsed), 1);
    let r1 = parsed.iter().find(|i| i.id == "R1").expect("R1");
    assert_eq!(r1.severity, IssueSeverity::Blocking);
    let r2 = parsed.iter().find(|i| i.id == "R2").expect("R2");
    assert_eq!(r2.severity, IssueSeverity::Residual);
    let r3 = parsed.iter().find(|i| i.id == "R3").expect("R3");
    assert_eq!(r3.severity, IssueSeverity::Residual);
    let (blocked, n) = gate_counts_after_residual_demote(&parsed, Some(0), false);
    assert!(blocked && n == 1, "intent gap must block gate; n={n}");
}
