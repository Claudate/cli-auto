//! Domain layer (A1 · P2-17).
//!
//! [INPUT]: 无 IO（适配器在 `plan` / `runtime` / `services/chat` / 未来 `adapters`）
//! [OUTPUT]: 纯模型与纯函数 — `plan` · `run` · `worker` · `inspect` · `chat`
//! [POS]: 六边形内核；**禁止**依赖 tauri / clap / UI / 具体 provider 实现
//! [PROTOCOL]: 变更时更新此头部与 src/CLAUDE.md；扩 split 时增子模块

/// Plan document model, validation, materialize, tag soft-routing.
pub mod plan;

/// Run status machine, retry/failover pure rules, active-set filters (A1-3).
pub mod run;

/// Worker route / isolation / failover policy objects (A1-4).
pub mod worker;

/// Inspect VERDICT/ISSUES parse + pure gate rules (A1-5).
pub mod inspect;

/// Chat pure rules: fence/title/normalize/stream parse (A1-6). **No** path join / fs.
pub mod chat;

/// A0 baseline marker (kept for skeleton smoke tests).
pub const A0_BASELINE: &str = "domain-a0";

#[cfg(test)]
mod tests {
    #[test]
    fn a0_domain_skeleton_loads() {
        assert_eq!(super::A0_BASELINE, "domain-a0");
    }

    #[test]
    fn plan_types_reachable() {
        assert_eq!(super::plan::MAX_TASKS, 23);
        assert_eq!(super::plan::PLANNER_MAX_TASKS, 20);
    }

    #[test]
    fn run_rules_reachable() {
        use super::run::{classify_retry, resolve_final_run_status, FinalRunStatus, RetryKind};
        assert_eq!(
            classify_retry("stopped", 1, 3, false, true),
            RetryKind::Permanent
        );
        assert_eq!(
            resolve_final_run_status(true, false, false),
            FinalRunStatus::Aborted
        );
    }

    #[test]
    fn worker_policy_reachable() {
        use super::worker::{isolation_on_fail, FailoverPolicy, IsolationOnFail, ProviderId};
        assert_eq!(ProviderId::parse("claude"), Some(ProviderId::Claude));
        assert_eq!(isolation_on_fail(true), IsolationOnFail::FailClosed);
        assert_eq!(
            FailoverPolicy::new(true, 1)
                .target_for("claude", &[])
                .as_deref(),
            Some("codex")
        );
    }

    #[test]
    fn inspect_rules_reachable() {
        use super::inspect::{parse_verdict_text, InspectVerdict, REWORK_MAX_ROUNDS};
        assert_eq!(parse_verdict_text("PASS\nok"), InspectVerdict::Pass);
        assert_eq!(REWORK_MAX_ROUNDS, 2);
    }

    #[test]
    fn chat_rules_reachable() {
        use super::chat::{extract_plan_fence, sanitize_session_id, truncate_chars};
        assert_eq!(
            extract_plan_fence("```plan\n# T\n```\n").as_deref(),
            Some("# T")
        );
        assert_eq!(sanitize_session_id("../x"), "___x");
        assert_eq!(truncate_chars("abcd", 3), "abc…");
    }
}
