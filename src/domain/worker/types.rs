//! Pure worker identity / route / capability flags (A1-4).
//!
//! [INPUT]: string labels from PlanIR / TaskIR
//! [OUTPUT]: typed ids for policy code (not wire schema)
//! [POS]: domain/worker — wire still uses String provider on TaskIR/run.json
//! [PROTOCOL]: 勿改 cco-run/v1 磁盘字段名

use crate::domain::plan::{TaskRole, TaskScope};

/// Known production / test provider ids. Unknown strings stay as raw names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Claude,
    Codex,
    Fake,
    /// Non-CLI path (P2-7 S0 inline / later Messages API). Registry opt-in.
    Sdk,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Fake => "fake",
            Self::Sdk => "sdk",
        }
    }

    /// Parse known ids; unknown → None (callers keep raw string).
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "fake" | "mock" => Some(Self::Fake),
            "sdk" | "claude-sdk" | "claude_sdk" => Some(Self::Sdk),
            _ => None,
        }
    }
}

/// Task route view (provider + optional collab role/scope).
/// PlanIR still owns storage; this is a pure projection for policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRoute {
    pub provider: String,
    pub role: Option<TaskRole>,
    pub scope: Option<TaskScope>,
}

impl WorkerRoute {
    pub fn from_task_fields(
        provider: impl Into<String>,
        role: Option<TaskRole>,
        scope: Option<TaskScope>,
    ) -> Self {
        Self {
            provider: provider.into(),
            role,
            scope,
        }
    }
}

/// Capability matrix flags (pure mirror of port Capabilities; no IO).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapabilityFlags {
    pub print: bool,
    pub background: bool,
    pub stop: bool,
    pub cost: bool,
    pub session_resume: bool,
    pub interactive_pty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_providers() {
        assert_eq!(ProviderId::parse("claude"), Some(ProviderId::Claude));
        assert_eq!(ProviderId::parse("CODEX"), Some(ProviderId::Codex));
        assert_eq!(ProviderId::parse("mock"), Some(ProviderId::Fake));
        assert_eq!(ProviderId::parse("sdk"), Some(ProviderId::Sdk));
        assert_eq!(ProviderId::parse("claude-sdk"), Some(ProviderId::Sdk));
        assert_eq!(ProviderId::as_str(ProviderId::Sdk), "sdk");
        assert_eq!(ProviderId::parse("other"), None);
    }
}
