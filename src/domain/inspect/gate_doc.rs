//! Machine inspect gate document (cco-inspect-gate/v1).
//!
//! Host SoT for terminal PASS/FAIL — not markdown prose.
//!
//! [INPUT]: JSON body string
//! [OUTPUT]: InspectGateDoc · InspectVerdict + blocking counts
//! [POS]: domain/inspect — pure; no fs
//! [PROTOCOL]: schema 变更须同步 inspect 系统提示与 materialize outputs

use serde::{Deserialize, Serialize};

use super::types::InspectVerdict;

pub const INSPECT_GATE_SCHEMA: &str = "cco-inspect-gate/v1";

/// Machine gate product written by inspect (and readable by host without md guesswork).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectGateDoc {
    #[serde(default = "default_schema")]
    pub schema: String,
    /// `pass` | `fail` (case-insensitive).
    pub result: String,
    /// Open blocking ISSUES count (gate-blocking).
    #[serde(default)]
    pub blocking: u32,
    /// Open map ISSUES count (gate-blocking).
    #[serde(default)]
    pub map: u32,
    /// Open residual count (does not block PASS).
    #[serde(default)]
    pub residual: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn default_schema() -> String {
    INSPECT_GATE_SCHEMA.into()
}

impl InspectGateDoc {
    pub fn verdict(&self) -> InspectVerdict {
        match self.result.trim().to_ascii_lowercase().as_str() {
            "pass" | "ok" | "success" => InspectVerdict::Pass,
            "fail" | "failed" | "error" => InspectVerdict::Fail,
            _ => InspectVerdict::Unknown,
        }
    }

    /// Issues that block plan-loop success.
    pub fn gate_blocking_n(&self) -> usize {
        (self.blocking as usize).saturating_add(self.map as usize)
    }

    pub fn pass_ok(&self) -> bool {
        matches!(self.verdict(), InspectVerdict::Pass) && self.gate_blocking_n() == 0
    }
}

/// Parse GATE.json body. Invalid JSON / missing result → None (caller falls back to md).
pub fn parse_gate_json(text: &str) -> Option<InspectGateDoc> {
    let doc: InspectGateDoc = serde_json::from_str(text.trim()).ok()?;
    if doc.result.trim().is_empty() {
        return None;
    }
    Some(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_pass_zero_blocking() {
        let doc = parse_gate_json(
            r#"{"schema":"cco-inspect-gate/v1","result":"pass","blocking":0,"map":0,"residual":4}"#,
        )
        .unwrap();
        assert_eq!(doc.verdict(), InspectVerdict::Pass);
        assert_eq!(doc.gate_blocking_n(), 0);
        assert!(doc.pass_ok());
    }

    #[test]
    fn gate_pass_with_blocking_not_ok() {
        let doc = parse_gate_json(r#"{"result":"pass","blocking":1,"map":0}"#).unwrap();
        assert!(!doc.pass_ok());
        assert_eq!(doc.gate_blocking_n(), 1);
    }

    #[test]
    fn gate_fail() {
        let doc = parse_gate_json(r#"{"result":"FAIL","blocking":2}"#).unwrap();
        assert_eq!(doc.verdict(), InspectVerdict::Fail);
    }
}
