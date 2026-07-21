//! Failover policy object (A1-4). Target names pure; live preflight stays in scheduler.
//!
//! [INPUT]: enabled · fallback_extra · current provider · retry counters
//! [OUTPUT]: optional target name · attempt budget · RetryKind
//! [POS]: domain/worker — wraps domain/run retry pure rules
//! [PROTOCOL]: manual stop never failovers (via classify_retry); fake has no production pair

use crate::domain::run::{
    attempt_budget, classify_retry, production_failover_target, RetryKind,
};

/// Pluggable failover policy (claude↔codex production pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailoverPolicy {
    pub enabled: bool,
    /// Extra attempts on the fallback provider after a switch (capped at 10 by attempt_budget).
    pub fallback_extra_attempts: u32,
}

impl FailoverPolicy {
    pub fn new(enabled: bool, fallback_extra_attempts: u32) -> Self {
        Self {
            enabled,
            fallback_extra_attempts,
        }
    }

    /// Pure name of production failover peer, or None if disabled / no peer.
    pub fn target_for(&self, current: &str) -> Option<&'static str> {
        if !self.enabled {
            return None;
        }
        production_failover_target(current)
    }

    pub fn attempt_budget(&self, failover_used: bool, same_provider_budget: u32) -> u32 {
        attempt_budget(
            failover_used,
            same_provider_budget,
            self.fallback_extra_attempts,
        )
    }

    pub fn classify(
        &self,
        reason_code: &str,
        attempt: u32,
        budget: u32,
        failover_used: bool,
    ) -> RetryKind {
        classify_retry(
            reason_code,
            attempt,
            budget,
            failover_used,
            self.enabled,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_yields_no_target() {
        let p = FailoverPolicy::new(false, 1);
        assert_eq!(p.target_for("claude"), None);
    }

    #[test]
    fn enabled_claude_codex_pair() {
        let p = FailoverPolicy::new(true, 1);
        assert_eq!(p.target_for("claude"), Some("codex"));
        assert_eq!(p.target_for("codex"), Some("claude"));
        assert_eq!(p.target_for("fake"), None);
    }

    #[test]
    fn stop_is_permanent() {
        let p = FailoverPolicy::new(true, 1);
        assert_eq!(p.classify("stopped", 1, 3, false), RetryKind::Permanent);
    }
}
