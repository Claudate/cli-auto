//! Pure VERDICT / ISSUES text parsers (A1-5).
//!
//! Split by pure-function boundary (arch hard ≤600):
//! - [`verdict`] — structured Result/VERDICT head scan
//! - [`issues`] — ISSUES body → graded rows
//!
//! [INPUT]: raw product file body strings
//! [OUTPUT]: InspectVerdict · Vec\<ParsedIssue\>
//! [POS]: domain/inspect — no filesystem
//! [PROTOCOL]: 解析语义变更须同步 tests + plan-execute-inspect 契约说明

mod issues;
mod verdict;

pub use issues::parse_issues_text;
pub use verdict::parse_verdict_text;
