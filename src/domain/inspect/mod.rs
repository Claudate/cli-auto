//! Inspect domain: pure VERDICT / ISSUES parse + gate rules (A1-5 · P2-17).
//!
//! ## Pure vs disk IO
//! | Pure (this module) | IO (runtime/handoff adapter) |
//! |--------------------|------------------------------|
//! | InspectVerdict · IssueSeverity · ParsedIssue | read/write `.cco-out/inspect/*` |
//! | parse_verdict_text · parse_issues_text | resolve_output_path · git CHANGED |
//! | candidate paths · count_blocking · gate fail reason | handoff board / fragments |
//! | push_inspect_gate_decision · can_start_rework | system_push path checks |
//! | rework_placeholder_note · MAP whitelist constants | build_rework_plan PlanIR + validate |
//!
//! [INPUT]: raw product text · TaskIR role/outputs · pre-parsed counts
//! [OUTPUT]: graded verdict/issues · pure gate decisions
//! [POS]: domain/inspect — **禁止**路径拼接 / fs / git / provider
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；磁盘 schema 不静默改

mod classify;
mod gate;
mod gate_doc;
mod parse;
mod types;

pub use classify::{
    all_blocking_are_docs_closeout, classify_kind, demote_residual_evidence_issues,
    effective_blocking_count, gate_counts_after_residual_demote, is_docs_closeout_issue,
    is_residual_evidence_gap, is_usability_blocking_issue, GapKind, DOCS_CLOSEOUT_PATH_HINTS,
};
pub use gate::{
    can_start_rework, count_blocking_issues, count_residual_issues, gate_candidate_paths,
    inspect_gate_fail_reason, inspect_pass_blocked, issues_candidate_paths, looks_like_issues_path,
    looks_like_verdict_path, push_inspect_gate_decision, rework_placeholder_note,
    task_has_verdict_gate, verdict_candidate_paths,
};
pub use gate_doc::{parse_gate_json, InspectGateDoc, INSPECT_GATE_SCHEMA};
pub use parse::{parse_issues_text, parse_verdict_text};
pub use types::{
    InspectVerdict, IssueSeverity, ParsedIssue, INSPECT_GATE_REL, INSPECT_ISSUES_REL,
    INSPECT_VERDICT_REL, MAP_REWORK_PATH_WHITELIST, REWORK_MAX_ROUNDS,
};
