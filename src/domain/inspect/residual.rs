//! Residual / usability grade normalize for host gate (P-loop).
//!
//! [INPUT]: ParsedIssue rows from ISSUES
//! [OUTPUT]: demote handwalk · promote real usability · effective blocking counts
//! [POS]: domain/inspect — pure; no fs / git
//! [PROTOCOL]: 否定句不得升 blocking；意图静默降级升 blocking；样本变更同步单测（含 test/9 R1/R3）

use super::types::{IssueSeverity, ParsedIssue};

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

/// Usability / anti-common-sense product failures — always host-blocking.
///
/// Even if an agent labels these residual (or mixes them with handwalk wording),
/// the run must not PASS. Complements implement usability floor prompts.
const USABILITY_BLOCKING_TOKENS: &[&str] = &[
    "主路径不可用",
    "主路径不通",
    "main path unusable",
    "main-path fail",
    "反常识",
    "anti-common",
    "新建即已",
    "新建已完成",
    "create already",
    "一点改多",
    "点 a 改 b",
    "mutates other",
    "mutates others",
    "串对象",
    "操作串",
    "隔离失败",
    "isolation fail",
    "丢数据",
    "数据丢失",
    "save/load loses",
    "刷新丢失",
    "状态错",
    "wrong state",
    "smoke ok:false",
    "smoke main-path ok:false",
    "功能 smoke 失败",
];

/// Plan-intent silent degradation — always host-blocking (not residual polish).
///
/// Agents often label “true-photo plan → illustrative SVG only” as residual and
/// PASS. Product rule: weakening a required success criterion without user accept
/// is blocking; rework must restore intent (e.g. search/download real images),
/// not clear unused scaffold CSS.
const INTENT_DEGRADATION_BLOCKING_TOKENS: &[&str] = &[
    "not photo stock",
    "illustrative svg",
    "illustrative svgs",
    "illustrative only",
    "geometry svg",
    "geometric svg",
    "静默降级",
    "意图降级",
    "验收降级",
    "更弱定义",
    "改弱定义",
    "改弱验收",
    "削弱验收",
    "silent degrad",
    "acceptance degrad",
    "degraded acceptance",
    "intent degrad",
    "插画顶真图",
    "插画冒充",
    "svg 冒充",
    "svg冒充",
    "非照片顶真实感",
    "非真实感商品图",
];

fn issue_haystack(issue: &ParsedIssue) -> String {
    format!(
        "{} {} {} {} {}",
        issue.id, issue.symptom, issue.fix_wp, issue.plan_ref, issue.raw
    )
}

fn hay_hits(hay: &str, tokens: &[&str]) -> bool {
    let hay_l = hay.to_ascii_lowercase();
    tokens.iter().any(|tok| {
        let t = tok.to_ascii_lowercase();
        hay_l.contains(&t) || hay.contains(*tok)
    })
}

/// Local negation window before a token match (bytes, then floored to char).
/// Covers `does not affect main path or wrong state` without whole-body scan.
const USABILITY_NEGATION_WINDOW: usize = 56;

/// Prefixes / phrases that mean the following usability token is **denied**.
const USABILITY_NEGATION_CUES: &[&str] = &[
    "does not",
    "doesn't",
    "doesnt",
    "do not",
    "don't",
    "dont",
    "did not",
    "didn't",
    "didnt",
    "without",
    "never",
    "no ",
    "not ",
    "nor ",
    "不影响",
    "不会",
    "没有",
    "并非",
    "无",
    "未",
    "不导致",
    "不引起",
    "不产生",
    "不到",
];

/// Floor `idx` to a char boundary in `s` (safe for CJK haystacks).
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// True when `before` (text immediately preceding a token hit) locally negates it.
fn usability_match_is_negated(before: &str) -> bool {
    let b = before.to_ascii_lowercase();
    let cut = floor_char_boundary(&b, b.len().saturating_sub(USABILITY_NEGATION_WINDOW));
    let tail = &b[cut..];
    USABILITY_NEGATION_CUES
        .iter()
        .any(|cue| tail.contains(&cue.to_ascii_lowercase()) || before.contains(*cue))
}

/// Usability token hit that is **not** under a local negation.
///
/// Prevents residual hygiene like「does not affect … or wrong state」from
/// promoting to blocking (P-loop false rework on PASS residual).
fn usability_token_hits(hay: &str, tokens: &[&str]) -> bool {
    // Lowercasing leaves CJK intact; one pass covers EN + 中文 tokens.
    let hay_l = hay.to_ascii_lowercase();
    for tok in tokens {
        let needle = tok.to_ascii_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut start = 0;
        while let Some(rel) = hay_l[start..].find(&needle) {
            let abs = start + rel;
            let window_start =
                floor_char_boundary(&hay_l, abs.saturating_sub(USABILITY_NEGATION_WINDOW));
            let before = &hay_l[window_start..abs];
            if !usability_match_is_negated(before) {
                return true;
            }
            start = abs + needle.len().max(1);
            if start <= abs {
                start = abs + 1;
                while start < hay_l.len() && !hay_l.is_char_boundary(start) {
                    start += 1;
                }
            }
        }
    }
    false
}

/// True when ISSUE describes unusable product / anti-common-sense behavior,
/// **or** silent degradation of a plan success criterion (intent downgrade).
///
/// Host keeps or promotes these to blocking; they must never be residual-only PASS.
/// Negated usability mentions (`does not … wrong state` / `不影响主路径`) do **not**
/// count. Intent-degradation phrases are matched as whole tokens (include their
/// own “not …” wording, e.g. `not photo stock`).
pub fn is_usability_blocking_issue(issue: &ParsedIssue) -> bool {
    if issue.severity == IssueSeverity::OutOfScope {
        return false;
    }
    let hay = issue_haystack(issue);
    if usability_token_hits(&hay, USABILITY_BLOCKING_TOKENS) {
        return true;
    }
    // Full-phrase intent tokens — no local-negation window (phrase may start with "not ").
    hay_hits(&hay, INTENT_DEGRADATION_BLOCKING_TOKENS)
}

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
}
