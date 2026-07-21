//! Isolation policy decisions (A1-4). Path layout stays in runtime/worktree.
//!
//! [INPUT]: iterator of task.provider strings · want_worktree flag (caller)
//! [OUTPUT]: IsolationOnFail · multi-provider detection
//! [POS]: domain/worker — scheduler maps to worktree IO on_fail
//! [PROTOCOL]: multi-provider mix → FailClosed (no silent shared cwd)

use std::collections::HashSet;

/// What the orchestrator should do when worktree create fails while wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationOnFail {
    /// Single-provider legacy: fall back to shared project root.
    #[default]
    FallbackSharedRoot,
    /// Multi-provider mix-run: surface error (task Failed).
    FailClosed,
}

/// True when the plan uses more than one distinct `task.provider`.
pub fn is_multi_provider<'a, I>(providers: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    let set: HashSet<&str> = providers.into_iter().collect();
    set.len() > 1
}

/// Isolation fail policy: mix-run fail-closed; single-provider may soft-fallback.
pub fn isolation_on_fail(multi_provider: bool) -> IsolationOnFail {
    if multi_provider {
        IsolationOnFail::FailClosed
    } else {
        IsolationOnFail::FallbackSharedRoot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_provider_detection() {
        assert!(!is_multi_provider(["claude", "claude"]));
        assert!(!is_multi_provider(["fake"]));
        assert!(is_multi_provider(["claude", "codex"]));
        assert!(is_multi_provider(["claude", "fake", "codex"]));
    }

    #[test]
    fn isolation_policy_matches_mix() {
        assert_eq!(
            isolation_on_fail(true),
            IsolationOnFail::FailClosed
        );
        assert_eq!(
            isolation_on_fail(false),
            IsolationOnFail::FallbackSharedRoot
        );
    }
}
