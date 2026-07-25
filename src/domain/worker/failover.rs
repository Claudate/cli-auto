//! Failover policy object (A1-4). Target names pure; live preflight stays in scheduler.
//!
//! [INPUT]: enabled · fallback_extra · order · current · tried · retry counters
//! [OUTPUT]: optional target name · attempt budget · RetryKind
//! [POS]: domain/worker — wraps domain/run retry pure rules
//! [PROTOCOL]: manual stop never failovers (via classify_retry); fake/sdk never auto peers

use crate::domain::run::{
    attempt_budget, classify_retry, default_failover_order, next_failover_target, RetryKind,
};

/// Pluggable failover policy (configurable order; default claude then codex).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailoverPolicy {
    pub enabled: bool,
    /// Extra attempts on each fallback provider after a switch (capped at 10 by attempt_budget).
    pub fallback_extra_attempts: u32,
    /// Production failover walk order (names). Empty → default claude,codex.
    pub order: Vec<String>,
}

impl FailoverPolicy {
    pub fn new(enabled: bool, fallback_extra_attempts: u32) -> Self {
        Self {
            enabled,
            fallback_extra_attempts,
            order: default_failover_order(),
        }
    }

    pub fn with_order(
        enabled: bool,
        fallback_extra_attempts: u32,
        order: Vec<String>,
    ) -> Self {
        let order = if order.is_empty() {
            default_failover_order()
        } else {
            order
        };
        Self {
            enabled,
            fallback_extra_attempts,
            order,
        }
    }

    /// Pure next failover peer, or None if disabled / exhausted.
    pub fn target_for(&self, current: &str, already_tried: &[String]) -> Option<String> {
        if !self.enabled {
            return None;
        }
        next_failover_target(current, &self.order, already_tried)
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
        current: &str,
        already_tried: &[String],
    ) -> RetryKind {
        let has_next = self.target_for(current, already_tried).is_some();
        classify_retry(
            reason_code,
            attempt,
            budget,
            has_next,
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
        assert_eq!(p.target_for("claude", &[]), None);
    }

    #[test]
    fn enabled_default_claude_codex_pair() {
        let p = FailoverPolicy::new(true, 1);
        assert_eq!(p.target_for("claude", &[]).as_deref(), Some("codex"));
        assert_eq!(p.target_for("codex", &[]).as_deref(), Some("claude"));
        assert_eq!(p.target_for("fake", &[]), None);
    }

    #[test]
    fn multi_order_chain() {
        let p = FailoverPolicy::with_order(
            true,
            1,
            vec!["claude".into(), "gemini".into(), "qwen".into()],
        );
        assert_eq!(p.target_for("claude", &[]).as_deref(), Some("gemini"));
        assert_eq!(
            p.target_for("claude", &["gemini".into()]).as_deref(),
            Some("qwen")
        );
        assert_eq!(
            p.target_for("claude", &["gemini".into(), "qwen".into()]),
            None
        );
    }

    #[test]
    fn stop_is_permanent() {
        let p = FailoverPolicy::new(true, 1);
        assert_eq!(
            p.classify("stopped", 1, 3, "claude", &[]),
            RetryKind::Permanent
        );
    }
}
