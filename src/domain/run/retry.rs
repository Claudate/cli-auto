//! Pure retry / failover policy (H4).
//!
//! [INPUT]: reason_code · attempt · budgets · failover flags · order · tried
//! [OUTPUT]: RetryKind · next failover target name
//! [POS]: domain/run — no preflight IO (caller checks provider alive)
//! [PROTOCOL]: 变更时更新 domain/run/mod.rs

/// User-initiated stop, success, and **inspect semantic FAIL** never auto-retry / failover.
///
/// `inspect_fail` = worker finished but host VERDICT gate rejected Done (P-loop).
/// Re-running the same inspect cannot clear blocking ISSUES; use rework wave instead.
pub fn is_non_retryable(reason_code: &str) -> bool {
    matches!(reason_code, "stopped" | "ok" | "inspect_fail")
}

/// Platform/API error (broken endpoint, 429, auth failure) — never retry same provider
/// (the endpoint is broken, not the task), but still eligible for failover to a healthy peer.
///
/// Recognizes the legacy `"platform_error"` reason code **and** the classified kinds
/// (`auth_invalid` / `insufficient_funds` / `rate_limited` / `endpoint_broken`) emitted by
/// [`PlatformErrorKind::reason_str`](crate::runtime::provider::shell_print::decode::PlatformErrorKind::reason_str).
pub fn is_platform_error(reason_code: &str) -> bool {
    matches!(
        reason_code,
        "platform_error" | "auth_invalid" | "insufficient_funds" | "rate_limited" | "endpoint_broken"
    )
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

/// Default production order when config omits `failover_order` (compat).
pub fn default_failover_order() -> Vec<String> {
    vec!["claude".into(), "codex".into()]
}

/// Providers never chosen by automatic failover (unless explicitly listed — still skipped).
pub fn is_non_failover_provider(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "fake" | "mock" | "sdk" | "claude-sdk" | "claude_sdk"
    )
}

/// Next production failover target from a configured order.
///
/// Walks `order` and returns the first name that is:
/// - non-empty
/// - not equal to `current` (case-insensitive)
/// - not in `already_tried` (case-insensitive)
/// - not fake/sdk
///
/// Does **not** wrap around to the head of the list (avoids burn loops).
pub fn next_failover_target(
    current: &str,
    order: &[String],
    already_tried: &[String],
) -> Option<String> {
    let cur = current.trim().to_ascii_lowercase();
    if cur.is_empty() || is_non_failover_provider(&cur) {
        return None;
    }
    let tried: Vec<String> = already_tried
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    for cand in order {
        let n = cand.trim().to_ascii_lowercase();
        if n.is_empty() || n == cur {
            continue;
        }
        if is_non_failover_provider(&n) {
            continue;
        }
        if tried.iter().any(|t| t == &n) {
            continue;
        }
        return Some(n);
    }
    None
}

/// Legacy pair helper: claude↔codex only. Prefer [`next_failover_target`] with config order.
pub fn production_failover_target(current: &str) -> Option<&'static str> {
    match current.trim().to_ascii_lowercase().as_str() {
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
    /// Same-provider budget exhausted; try next name in failover_order
    /// (caller still runs preflight and may fall through to Permanent).
    TryFailover,
    /// Exhausted or non-retryable → permanent terminal.
    Permanent,
}

/// Classify retry outcome.
///
/// `has_next_failover`: pure — caller computed `next_failover_target(...).is_some()`.
pub fn classify_retry(
    reason_code: &str,
    attempt: u32,
    budget: u32,
    has_next_failover: bool,
    failover_enabled: bool,
) -> RetryKind {
    if is_non_retryable(reason_code) {
        return RetryKind::Permanent;
    }
    // Platform error: endpoint is broken, not the task. Skip same-provider retry entirely.
    if is_platform_error(reason_code) {
        if failover_enabled && has_next_failover {
            return RetryKind::TryFailover;
        }
        return RetryKind::Permanent;
    }
    if attempt <= budget {
        return RetryKind::SameProvider;
    }
    if failover_enabled && has_next_failover {
        return RetryKind::TryFailover;
    }
    RetryKind::Permanent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_error_reasons_skip_same_provider() {
        // Legacy literal still recognized.
        assert!(is_platform_error("platform_error"));
        // Classified kinds (PlatformErrorKind::reason_str) also recognized.
        assert!(is_platform_error("auth_invalid"));
        assert!(is_platform_error("insufficient_funds"));
        assert!(is_platform_error("rate_limited"));
        assert!(is_platform_error("endpoint_broken"));
        // Ordinary failures are not platform errors.
        assert!(!is_platform_error("fail"));
        assert!(!is_platform_error("timeout"));

        // Platform error → TryFailover (not SameProvider) when a peer is available.
        for reason in ["auth_invalid", "insufficient_funds", "rate_limited", "endpoint_broken"] {
            assert_eq!(
                classify_retry(reason, 1, 3, true, true),
                RetryKind::TryFailover,
                "{reason} should skip same-provider retry"
            );
        }
        // No failover available → Permanent (never same-provider retry a broken endpoint).
        assert_eq!(
            classify_retry("auth_invalid", 1, 3, false, true),
            RetryKind::Permanent
        );
    }

    #[test]
    fn stop_never_retries() {
        assert!(is_non_retryable("stopped"));
        assert!(!can_same_provider_retry("stopped", 1, 3));
        assert_eq!(
            classify_retry("stopped", 1, 3, true, true),
            RetryKind::Permanent
        );
    }

    #[test]
    fn inspect_fail_never_retries_or_failovers() {
        assert!(is_non_retryable("inspect_fail"));
        assert_eq!(
            classify_retry("inspect_fail", 1, 3, true, true),
            RetryKind::Permanent
        );
    }

    #[test]
    fn same_provider_then_failover_then_permanent() {
        assert_eq!(
            classify_retry("fail", 1, 2, true, true),
            RetryKind::SameProvider
        );
        assert_eq!(
            classify_retry("fail", 3, 2, true, true),
            RetryKind::TryFailover
        );
        assert_eq!(
            classify_retry("fail", 3, 2, false, true),
            RetryKind::Permanent
        );
        assert_eq!(
            classify_retry("fail", 3, 2, true, false),
            RetryKind::Permanent
        );
    }

    #[test]
    fn failover_targets_claude_codex_legacy() {
        assert_eq!(production_failover_target("claude"), Some("codex"));
        assert_eq!(production_failover_target("codex"), Some("claude"));
        assert_eq!(production_failover_target("fake"), None);
    }

    #[test]
    fn next_failover_walks_order_skips_tried_and_fake() {
        let order = vec![
            "claude".into(),
            "codex".into(),
            "gemini".into(),
            "fake".into(),
            "qwen".into(),
        ];
        assert_eq!(
            next_failover_target("claude", &order, &[]).as_deref(),
            Some("codex")
        );
        assert_eq!(
            next_failover_target("claude", &order, &["codex".into()]).as_deref(),
            Some("gemini")
        );
        assert_eq!(
            next_failover_target(
                "claude",
                &order,
                &["codex".into(), "gemini".into(), "qwen".into()]
            ),
            None
        );
        // current skipped; fake never chosen
        assert_eq!(
            next_failover_target("codex", &order, &[]).as_deref(),
            Some("claude")
        );
    }

    #[test]
    fn next_failover_no_wrap() {
        let order = vec!["claude".into(), "codex".into()];
        assert_eq!(
            next_failover_target("codex", &order, &["claude".into()]),
            None
        );
    }

    #[test]
    fn attempt_budget_switches_after_failover() {
        assert_eq!(attempt_budget(false, 3, 1), 3);
        assert_eq!(attempt_budget(true, 3, 1), 1);
    }
}
