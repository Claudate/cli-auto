//! Usability / anti-common-sense + intent-degradation promote (pure).
//!
//! Split by concern boundary (arch soft ≤400).

use super::super::types::{IssueSeverity, ParsedIssue};
use super::{hay_hits, issue_haystack};

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
