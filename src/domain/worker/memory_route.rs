//! Memory-informed pre-spawn failover (P3 pilot · agentmemory-integration-plan-2026-08-12).
//!
//! [INPUT]: aggregated task-outcome history · current provider · candidate order
//! [OUTPUT]: preventive failover target + human reason (or None)
//! [POS]: domain/worker — pure decision; memory search IO stays in runtime/scheduler
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；仅预防性建议，不覆盖显式 route 语义
//!
//! Rule: with at least [`MEMORY_FAILOVER_MIN_SAMPLES`] recorded outcomes for the
//! same (provider, role) pair, a historical failure rate above
//! [`MEMORY_FAILOVER_MAX_FAIL_RATE`] switches to the first viable candidate.
//! Candidates must be pre-filtered by the caller (registered · healthy · ≠ current).

/// Aggregated history for one (provider, role) pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryOutcomeStats {
    pub failures: usize,
    pub total: usize,
}

impl MemoryOutcomeStats {
    /// Count one recorded outcome ("success" | "timeout" | "failed" | …).
    pub fn add_outcome(&mut self, outcome: &str) {
        self.total += 1;
        if matches!(outcome, "timeout" | "failed") {
            self.failures += 1;
        }
    }

    pub fn failure_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.failures as f32 / self.total as f32
        }
    }
}

/// Minimum recorded outcomes before history may influence routing.
pub const MEMORY_FAILOVER_MIN_SAMPLES: usize = 3;
/// Failure rate above which we preventively switch provider.
pub const MEMORY_FAILOVER_MAX_FAIL_RATE: f32 = 0.3;

/// Decide a preventive provider switch from recorded history.
///
/// `candidates` must already exclude the current provider, unhealthy providers
/// and providers not present in the registry (caller-side IO concerns).
/// Returns `(target, human_reason)` or `None` (insufficient data / healthy history / no candidate).
pub fn memory_failover_target(
    stats: &MemoryOutcomeStats,
    current: &str,
    candidates: &[String],
) -> Option<(String, String)> {
    if stats.total < MEMORY_FAILOVER_MIN_SAMPLES {
        return None;
    }
    let rate = stats.failure_rate();
    if rate <= MEMORY_FAILOVER_MAX_FAIL_RATE {
        return None;
    }
    let target = candidates.first()?.clone();
    let reason = format!(
        "历史失败率 {:.0}%（{}/{} 次）→ 预防性切换 {} → {}",
        rate * 100.0,
        stats.failures,
        stats.total,
        current,
        target
    );
    Some((target, reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(failures: usize, total: usize) -> MemoryOutcomeStats {
        MemoryOutcomeStats { failures, total }
    }

    #[test]
    fn below_min_samples_never_switches() {
        let s = stats(2, 2); // 100% fail but only 2 samples
        assert!(memory_failover_target(&s, "claude", &["codex".into()]).is_none());
    }

    #[test]
    fn low_failure_rate_stays() {
        let s = stats(1, 10); // 10% < 30%
        assert!(memory_failover_target(&s, "claude", &["codex".into()]).is_none());
    }

    #[test]
    fn boundary_rate_stays() {
        let s = stats(3, 10); // exactly 30% — not above threshold
        assert!(memory_failover_target(&s, "claude", &["codex".into()]).is_none());
    }

    #[test]
    fn high_failure_rate_switches_to_first_candidate() {
        let s = stats(3, 5); // 60%
        let (target, reason) =
            memory_failover_target(&s, "claude", &["codex".into(), "gemini".into()]).unwrap();
        assert_eq!(target, "codex");
        assert!(reason.contains("60%"), "reason={reason}");
        assert!(reason.contains("claude"), "reason={reason}");
        assert!(reason.contains("codex"), "reason={reason}");
    }

    #[test]
    fn no_candidate_no_switch() {
        let s = stats(3, 3);
        assert!(memory_failover_target(&s, "claude", &[]).is_none());
    }

    #[test]
    fn add_outcome_counts_failures() {
        let mut s = MemoryOutcomeStats::default();
        s.add_outcome("timeout");
        s.add_outcome("failed");
        s.add_outcome("success");
        s.add_outcome("stopped"); // manual stop is not a provider failure
        assert_eq!(s.total, 4);
        assert_eq!(s.failures, 2);
        assert!((s.failure_rate() - 0.5).abs() < f32::EPSILON);
    }
}
