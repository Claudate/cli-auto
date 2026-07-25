//! Inspect domain types (A1-5 · P2-17).
//!
//! ## Pure parse/gate vs disk IO
//! | Pure (this module) | IO (runtime/handoff adapter) |
//! |--------------------|------------------------------|
//! | InspectVerdict · IssueSeverity · ParsedIssue | read VERDICT.md / ISSUES.md |
//! | parse_verdict_text · parse_issues_text | resolve_output_path · missing_outputs |
//! | candidate path lists · count_blocking | system_push path read |
//! | rework placeholder text · gate reasons | handoff.json board write |
//!
//! [INPUT]: raw VERDICT/ISSUES text · TaskIR fields (role/outputs)
//! [OUTPUT]: graded verdict/issues · pure gate decisions
//! [POS]: domain/inspect — **no** path join / fs / git
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；**勿**静默改 handoff 磁盘 schema

use serde::{Deserialize, Serialize};

/// Conventional inspect verdict product (relative to work_dir / project_root).
/// Human-readable; host gate **prefers** [`INSPECT_GATE_REL`] when present.
pub const INSPECT_VERDICT_REL: &str = ".cco-out/inspect/VERDICT.md";
/// Conventional inspect issues product for rework consumption (P2-3).
/// Human-readable; host gate **prefers** [`INSPECT_GATE_REL`] when present.
pub const INSPECT_ISSUES_REL: &str = ".cco-out/inspect/ISSUES.md";
/// Machine gate product — **host SoT** for Pass/Fail + open blocking/map counts.
/// Schema: `{ "schema":"cco-inspect-gate/v1", "result":"pass"|"fail", "blocking":N, "map":N, "residual":N }`.
/// Markdown prose must not be the sole fail-closed input.
pub const INSPECT_GATE_REL: &str = ".cco-out/inspect/GATE.json";

/// Default max rework waves after inspect FAIL / blocking ISSUES (P-loop Q5).
pub const REWORK_MAX_ROUNDS: u32 = 2;

/// Map-class / docs-closeout rework may only touch GEB/docs pointers
/// (P-loop Q2/Q3 + Ensure E2; inspect still read-only).
pub const MAP_REWORK_PATH_WHITELIST: &[&str] = &[
    "CLAUDE.md",
    "docs/CLAUDE.md",
    "docs/gap-and-landing-plan-2026-07-18.md",
    "docs/plan-execute-inspect-rework-2026-07-19.md",
    "docs/inspect-ensure-close-loop-2026-07-24.md",
    "docs/**",
    "**/*.md",
    "README*",
    ".cco-out/**",
    ".cco-out/progress/**",
    "tests/**/README*",
];

/// Classification of inspect VERDICT product (P2-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectVerdict {
    Pass,
    Fail,
    /// No verdict file, or content does not clearly say PASS/FAIL.
    Unknown,
}

/// ISSUE severity grades (P-loop §3.4.3). `map` defaults to blocking for host gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Blocking,
    Map,
    Residual,
    OutOfScope,
}

impl IssueSeverity {
    /// Host gate: blocks plan-loop success unless residual/out-of-scope or user accepts residual.
    pub fn is_blocking_for_gate(self) -> bool {
        matches!(self, IssueSeverity::Blocking | IssueSeverity::Map)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            IssueSeverity::Blocking => "blocking",
            IssueSeverity::Map => "map",
            IssueSeverity::Residual => "residual",
            IssueSeverity::OutOfScope => "out-of-scope",
        }
    }
}

/// One structured ISSUES row (best-effort parse; free-form still works).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIssue {
    pub id: String,
    pub severity: IssueSeverity,
    pub plan_ref: String,
    pub path: String,
    pub symptom: String,
    pub fix_wp: String,
    /// Original line(s) for rework prompt paste.
    pub raw: String,
}
