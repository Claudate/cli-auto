//! Pure retry / failover policy (H4).
//!
//! [INPUT]: reason_code · attempt · budgets · failover flags
//! [OUTPUT]: RetryKind · failover target name
//! [POS]: domain/run — no preflight IO (caller checks provider alive)
//! [PROTOCOL]: 变更时更新 domain/run/mod.rs

/// User-initiated stop, success, and **inspect semantic FAIL** never auto-retry / failover.
///
/// `inspect_fail` = worker finished but host VERDICT gate rejected Done (P-loop).
/// Re-running the same inspect cannot clear blocking ISSUES; use rework wave instead.
pub fn is_non_retryable(reason_code: &str) -> bool {
    matches!(reason_code, "stopped" | "ok" | "inspect_fail")
}

/// Host inspect gate rewrote Done → Failed with a VERDICT/ISSUES reason.
pub fn is_inspect_gate_error(error: Option<&str>) -> bool {
    error
        .map(|e| e.contains("inspect VERDICT"))
        .unwrap_or(false)
}

/// Attempt budget for this task: after failover, only `fallback_extra`.
pub fn attempt_budget(failover_used: bool, same_provider_budget: u32, fallback_extra: u32) -> u32 {
    if failover_used {
        fallback_extra.min(10)
    } else {
        same_provider_budget
    }
}

/// plan.retry_max wins if higher; otherwise scheduler/config default. Cap 10.
pub fn effective_retry_max(plan_retry: u32, scheduler_retry: u32) -> u32 {
    plan_retry.max(scheduler_retry).min(10)
}

/// Same-provider auto-retry allowed?
pub fn can_same_provider_retry(reason_code: &str, attempt: u32, budget: u32) -> bool {
    !is_non_retryable(reason_code) && attempt <= budget
}

/// Production failover target: claude↔codex only. `fake` and others → None.
pub fn production_failover_target(current: &str) -> Option<&'static str> {
    match current {
        "claude" => Some("codex"),
        "codex" => Some("claude"),
        _ => None,
    }
}

/// What the orchestrator should do after a non-success finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryKind {
    /// Reset to Pending and re-queue on the same provider.
    SameProvider,
    /// Same-provider budget exhausted; try claude↔codex switch once
    /// (caller still runs preflight and may fall through to Permanent).
    TryFailover,
    /// Exhausted or non-retryable → permanent terminal.
    Permanent,
}

/// Classify retry outcome. Failover arm requires `failover_enabled` and
/// `!failover_used`; actual target resolution is separate (IO-free name only).
pub fn classify_retry(
    reason_code: &str,
    attempt: u32,
    budget: u32,
    failover_used: bool,
    failover_enabled: bool,
) -> RetryKind {
    if is_non_retryable(reason_code) {
        return RetryKind::Permanent;
    }
    if attempt <= budget {
        return RetryKind::SameProvider;
    }
    if !failover_used && failover_enabled {
        return RetryKind::TryFailover;
    }
    RetryKind::Permanent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_never_retries() {
        assert!(is_non_retryable("stopped"));
        assert!(!can_same_provider_retry("stopped", 1, 3));
        assert_eq!(
            classify_retry("stopped", 1, 3, false, true),
            RetryKind::Permanent
        );
    }

    #[test]
    fn inspect_fail_never_retries_or_failovers() {
        assert!(is_non_retryable("inspect_fail"));
        assert!(!can_same_provider_retry("inspect_fail", 1, 3));
        assert_eq!(
            classify_retry("inspect_fail", 1, 3, false, true),
            RetryKind::Permanent
        );
        assert!(is_inspect_gate_error(Some(
            "inspect VERDICT=FAIL (12 ISSUES line(s) for rework (Open risks ISSUES[t7-inspect]))"
        )));
        assert!(!is_inspect_gate_error(Some("env: node: No such file")));
        assert!(!is_inspect_gate_error(None));
    }

    #[test]
    fn same_provider_then_failover_then_permanent() {
        assert_eq!(
            classify_retry("fail", 1, 2, false, true),
            RetryKind::SameProvider
        );
        assert_eq!(
            classify_retry("fail", 3, 2, false, true),
            RetryKind::TryFailover
        );
        assert_eq!(
            classify_retry("fail", 3, 2, true, true),
            RetryKind::Permanent
        );
        assert_eq!(
            classify_retry("fail", 3, 2, false, false),
            RetryKind::Permanent
        );
    }

    #[test]
    fn failover_targets_claude_codex_only() {
        assert_eq!(production_failover_target("claude"), Some("codex"));
        assert_eq!(production_failover_target("codex"), Some("claude"));
        assert_eq!(production_failover_target("fake"), None);
    }

    #[test]
    fn attempt_budget_switches_after_failover() {
        assert_eq!(attempt_budget(false, 3, 1), 3);
        assert_eq!(attempt_budget(true, 3, 1), 1);
        assert_eq!(attempt_budget(true, 3, 99), 10);
    }

    #[test]
    fn effective_retry_cap() {
        assert_eq!(effective_retry_max(2, 5), 5);
        assert_eq!(effective_retry_max(8, 2), 8);
        assert_eq!(effective_retry_max(20, 20), 10);
    }
}
