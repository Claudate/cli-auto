//! Stamp TaskState route provenance from domain fill reports (P1-2)
//! and compose live human `route_label` (P1-3).
//!
//! [INPUT]: RunState · PlanIR · RouteFillReport / CostRouteReport · provider + route_*
//! [OUTPUT]: tasks.{id}.route_source|route_previous|route_note · route_label 人话
//! [POS]: app::run — **only** place that writes route_* onto RunState (not domain);
//!   live DTO labels also composed here (UI must not re-map raw enum)
//! [PROTOCOL]: last write wins; force → all Force; soft filled → SoftFill;
//!   soft kept + tag-implied match → TagRouting else Explicit; cost_auto / cost_escalate;
//!   failover elsewhere

use crate::domain::plan::tag_implied_provider;
use crate::domain::worker::{CostRouteReport, RouteFillMode, RouteFillReport};
use crate::plan::PlanIR;
use crate::state::{RouteSource, RunState};

/// Product-facing engine name (not raw CLI id). Shared by live / fail cards.
pub fn provider_product_label(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "claude" => "Claude".into(),
        "codex" => "Codex".into(),
        "fake" | "mock" => "演练".into(),
        "sdk" | "claude-sdk" | "claude_sdk" => "SDK".into(),
        "gemini" | "google" => "Gemini".into(),
        "qwen" | "tongyi" => "通义 Qwen".into(),
        "kimi" | "moonshot" => "Kimi".into(),
        // Channel id stays deepseek; product is CodeWhale (DeepSeek-native harness).
        "deepseek" | "codewhale" | "codew" => "CodeWhale".into(),
        "copilot" | "github-copilot" | "github_copilot" => "Copilot".into(),
        "codebuddy" | "tencent" | "cbc" => "CodeBuddy".into(),
        "" => "未知".into(),
        other => {
            // Keep unknown ids readable without inventing a brand.
            let mut chars = other.chars();
            match chars.next() {
                None => "未知".into(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

/// App-composed one-line route story for live / result / fail UI (P1-3).
///
/// UI prefixes with `执行方式：` when rendering. Never returns raw enum tags.
///
/// | source | label shape |
/// |--------|-------------|
/// | `None` (old run) | `{产品标签}` |
/// | explicit | `{产品} · 你在拆分台指定的` |
/// | soft_fill | `{产品} · 默认填充` |
/// | tag_routing | `{产品} · 按标签约定` |
/// | force | `{产品} · 强制指定` |
/// | failover | `{产品} · 故障切换前为 {先前产品}` |
/// | cost_auto | `{产品} · 费用优选` |
/// | cost_escalate | `{产品} · 失败后升档，先前 {prev}` |
/// | cost_budget | `{产品} · 预算收紧，先前 {prev}` |
pub fn compose_route_label(
    provider: &str,
    source: Option<RouteSource>,
    previous: Option<&str>,
) -> String {
    let product = provider_product_label(provider);
    match source {
        None => product,
        Some(RouteSource::Explicit) => format!("{product} · 你在拆分台指定的"),
        Some(RouteSource::SoftFill) => format!("{product} · 默认填充"),
        Some(RouteSource::TagRouting) => format!("{product} · 按标签约定"),
        Some(RouteSource::Force) => format!("{product} · 强制指定"),
        Some(RouteSource::CostAuto) => format!("{product} · 费用优选"),
        Some(RouteSource::Failover) => {
            let prev = previous
                .filter(|s| !s.trim().is_empty())
                .map(provider_product_label)
                .unwrap_or_else(|| "先前通道".into());
            format!("{product} · 故障切换前为 {prev}")
        }
        Some(RouteSource::CostEscalate) => {
            let prev = previous
                .filter(|s| !s.trim().is_empty())
                .map(provider_product_label)
                .unwrap_or_else(|| "先前通道".into());
            format!("{product} · 失败后升档，先前 {prev}")
        }
        Some(RouteSource::CostBudget) => {
            let prev = previous
                .filter(|s| !s.trim().is_empty())
                .map(provider_product_label)
                .unwrap_or_else(|| "先前通道".into());
            format!("{product} · 预算收紧，先前 {prev}")
        }
    }
}

/// Apply a soft/force fill report onto `state.tasks` (provider already on TaskState
/// from PlanIR). Safe to call multiple times; later reports overwrite earlier ones.
pub fn stamp_route_fill(state: &mut RunState, plan: &PlanIR, report: &RouteFillReport) {
    match report.mode {
        RouteFillMode::Force => {
            for id in &report.filled_ids {
                if let Some(ts) = state.tasks.get_mut(id) {
                    ts.route_source = Some(RouteSource::Force);
                    ts.route_previous = None;
                    ts.route_note = None;
                }
            }
        }
        RouteFillMode::Soft => {
            for id in &report.filled_ids {
                if let Some(ts) = state.tasks.get_mut(id) {
                    ts.route_source = Some(RouteSource::SoftFill);
                    ts.route_previous = None;
                    ts.route_note = None;
                }
            }
            for id in &report.kept_ids {
                let source = plan
                    .task(id)
                    .and_then(|t| {
                        let implied = tag_implied_provider(&t.tags)?;
                        if t.provider.eq_ignore_ascii_case(implied) {
                            Some(RouteSource::TagRouting)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(RouteSource::Explicit);
                if let Some(ts) = state.tasks.get_mut(id) {
                    ts.route_source = Some(source);
                    ts.route_previous = None;
                    ts.route_note = None;
                }
            }
        }
    }
}

/// When no fill report is available (legacy / already-resolved IR), infer a
/// conservative provenance from the final plan graph.
///
/// - tag-implied provider matches task → `tag_routing`
/// - task provider equals plan default → `soft_fill`
/// - else → `explicit`
pub fn stamp_route_inferred(state: &mut RunState, plan: &PlanIR) {
    let default = plan.default_provider.as_str();
    for t in &plan.tasks {
        let Some(ts) = state.tasks.get_mut(&t.id) else {
            continue;
        };
        if ts.route_source.is_some() {
            continue;
        }
        if let Some(implied) = tag_implied_provider(&t.tags) {
            if t.provider.eq_ignore_ascii_case(implied) {
                ts.route_source = Some(RouteSource::TagRouting);
                continue;
            }
        }
        if !default.is_empty() && t.provider.eq_ignore_ascii_case(default) {
            ts.route_source = Some(RouteSource::SoftFill);
        } else {
            ts.route_source = Some(RouteSource::Explicit);
        }
    }
}

/// Mid-run H4 failover: mark provider switch provenance on one task.
pub fn stamp_failover(
    state: &mut RunState,
    task_id: &str,
    previous_provider: &str,
    note: Option<&str>,
) {
    if let Some(ts) = state.tasks.get_mut(task_id) {
        ts.route_source = Some(RouteSource::Failover);
        ts.route_previous = Some(previous_provider.to_string());
        ts.route_note = note.map(|s| s.to_string());
    }
}

/// P0: stamp tasks rewritten by cost-aware routing (last write after soft/tag).
pub fn stamp_cost_route(state: &mut RunState, report: &CostRouteReport) {
    for c in &report.changed {
        if let Some(ts) = state.tasks.get_mut(&c.task_id) {
            ts.route_source = Some(RouteSource::CostAuto);
            ts.route_previous = None;
            ts.route_note = Some(c.rationale.clone());
            // Provider on TaskState may still be pre-cost; align with plan rewrite.
            ts.provider = c.to.clone();
        }
    }
}

/// P1: mid-run cost escalate (higher tier after failure).
pub fn stamp_cost_escalate(
    state: &mut RunState,
    task_id: &str,
    previous_provider: &str,
    note: Option<&str>,
) {
    if let Some(ts) = state.tasks.get_mut(task_id) {
        ts.route_source = Some(RouteSource::CostEscalate);
        ts.route_previous = Some(previous_provider.to_string());
        ts.route_note = note.map(|s| s.to_string());
    }
}

/// P2: mid-run budget ceiling forced a cheaper provider.
pub fn stamp_cost_budget(
    state: &mut RunState,
    task_id: &str,
    previous_provider: &str,
    note: Option<&str>,
) {
    if let Some(ts) = state.tasks.get_mut(task_id) {
        ts.route_source = Some(RouteSource::CostBudget);
        ts.route_previous = Some(previous_provider.to_string());
        ts.route_note = note.map(|s| s.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR};
    use crate::domain::worker::{apply_route_fill, apply_worker_defaults, RouteFillMode};
    use std::path::PathBuf;

    fn task(id: &str, provider: &str, tags: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: provider.into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: tags.iter().map(|s| (*s).to_string()).collect(),
            wait_for: vec![],
        }
    }

    fn mixed_plan() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "mixed".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("mixed.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                task("t1", "claude", &[]),
                task("t2", "codex", &[]),
                task("t3", "claude", &["codex"]), // will be tag-routed then kept
                task("t4", "default", &[]),
            ],
        }
    }

    #[test]
    fn soft_fill_stamps_mixed_explicit_and_soft() {
        let mut ir = mixed_plan();
        // Simulate prior tag routing on t3
        ir.tasks[2].provider = "codex".into();
        let report = apply_route_fill(&mut ir, "fake", RouteFillMode::Soft).unwrap();
        let mut state = RunState::new(
            "run-test".into(),
            PathBuf::from("/tmp/proj"),
            &ir,
            PathBuf::from("/tmp/run"),
        );
        stamp_route_fill(&mut state, &ir, &report);

        assert_eq!(state.tasks["t1"].route_source, Some(RouteSource::SoftFill));
        assert_eq!(state.tasks["t2"].route_source, Some(RouteSource::Explicit));
        // kept + tag-implied match → tag_routing
        assert_eq!(
            state.tasks["t3"].route_source,
            Some(RouteSource::TagRouting)
        );
        assert_eq!(state.tasks["t4"].route_source, Some(RouteSource::SoftFill));
    }

    #[test]
    fn force_stamps_all_force() {
        let mut ir = mixed_plan();
        let report = apply_route_fill(&mut ir, "fake", RouteFillMode::Force).unwrap();
        let mut state = RunState::new(
            "run-force".into(),
            PathBuf::from("/tmp/proj"),
            &ir,
            PathBuf::from("/tmp/run"),
        );
        stamp_route_fill(&mut state, &ir, &report);
        assert!(state
            .tasks
            .values()
            .all(|t| t.route_source == Some(RouteSource::Force)));
    }

    #[test]
    fn worker_defaults_report_stamps_like_soft() {
        let mut ir = mixed_plan();
        ir.tasks[2].provider = "codex".into();
        let report = apply_worker_defaults(&mut ir, "fake", "print");
        let mut state = RunState::new(
            "run-wd".into(),
            PathBuf::from("/tmp/proj"),
            &ir,
            PathBuf::from("/tmp/run"),
        );
        stamp_route_fill(&mut state, &ir, &report);
        assert_eq!(state.tasks["t2"].route_source, Some(RouteSource::Explicit));
        assert_eq!(
            state.tasks["t3"].route_source,
            Some(RouteSource::TagRouting)
        );
        assert_eq!(state.tasks["t1"].route_source, Some(RouteSource::SoftFill));
    }

    #[test]
    fn failover_sets_previous() {
        let ir = mixed_plan();
        let mut state = RunState::new(
            "run-fo".into(),
            PathBuf::from("/tmp/proj"),
            &ir,
            PathBuf::from("/tmp/run"),
        );
        stamp_failover(&mut state, "t1", "claude", Some("stall"));
        let ts = &state.tasks["t1"];
        assert_eq!(ts.route_source, Some(RouteSource::Failover));
        assert_eq!(ts.route_previous.as_deref(), Some("claude"));
        assert_eq!(ts.route_note.as_deref(), Some("stall"));
    }

    #[test]
    fn route_label_explicit_has_指定_semantic() {
        let label = compose_route_label("codex", Some(RouteSource::Explicit), None);
        assert!(label.contains("Codex"), "{label}");
        assert!(label.contains("指定"), "{label}");
        assert!(!label.contains("explicit"), "no raw enum: {label}");
    }

    #[test]
    fn route_label_soft_fill_has_默认_semantic() {
        let label = compose_route_label("claude", Some(RouteSource::SoftFill), None);
        assert!(label.contains("默认"), "{label}");
        assert!(!label.contains("soft_fill"), "no raw enum: {label}");
    }

    #[test]
    fn route_label_failover_mentions_previous_product() {
        let label = compose_route_label("codex", Some(RouteSource::Failover), Some("claude"));
        assert!(label.contains("故障切换"), "{label}");
        assert!(label.contains("Claude"), "{label}");
        assert!(label.contains("Codex"), "{label}");
    }

    #[test]
    fn route_label_cost_auto() {
        let label = compose_route_label("codex", Some(RouteSource::CostAuto), None);
        assert!(label.contains("费用优选"), "{label}");
        assert!(label.contains("Codex"), "{label}");
        assert!(!label.contains("cost_auto"), "{label}");
    }

    #[test]
    fn stamp_cost_route_overrides_soft_and_aligns_provider() {
        use crate::domain::worker::{CostRouteChange, CostRouteReport, CostTier};
        let ir = mixed_plan();
        let mut state = RunState::new(
            "run-cost".into(),
            PathBuf::from("/tmp/proj"),
            &ir,
            PathBuf::from("/tmp/run"),
        );
        // Pretend soft-fill already stamped.
        state.tasks.get_mut("t1").unwrap().route_source = Some(RouteSource::SoftFill);
        let report = CostRouteReport {
            changed: vec![CostRouteChange {
                task_id: "t1".into(),
                from: "claude".into(),
                to: "codex".into(),
                tier: CostTier::Mid,
                rationale: "费用优选·mid（中等费用）".into(),
            }],
            skipped_ids: vec![],
        };
        stamp_cost_route(&mut state, &report);
        let ts = &state.tasks["t1"];
        assert_eq!(ts.route_source, Some(RouteSource::CostAuto));
        assert_eq!(ts.provider, "codex");
        assert!(ts.route_note.as_deref().unwrap().contains("费用优选"));
    }

    #[test]
    fn route_label_cost_escalate() {
        let label = compose_route_label("claude", Some(RouteSource::CostEscalate), Some("codex"));
        assert!(label.contains("升档"), "{label}");
        assert!(label.contains("Codex"), "{label}");
        assert!(label.contains("Claude"), "{label}");
    }

    #[test]
    fn route_label_cost_budget() {
        let label = compose_route_label("codex", Some(RouteSource::CostBudget), Some("claude"));
        assert!(label.contains("预算收紧"), "{label}");
        assert!(label.contains("Claude"), "{label}");
        assert!(label.contains("Codex"), "{label}");
    }

    #[test]
    fn route_label_old_run_is_product_only() {
        let label = compose_route_label("claude", None, None);
        assert_eq!(label, "Claude");
    }

    #[test]
    fn provider_product_label_known() {
        assert_eq!(provider_product_label("claude"), "Claude");
        assert_eq!(provider_product_label("CODEX"), "Codex");
        assert_eq!(provider_product_label("fake"), "演练");
        assert_eq!(provider_product_label("gemini"), "Gemini");
        assert_eq!(provider_product_label("qwen"), "通义 Qwen");
        assert_eq!(provider_product_label("kimi"), "Kimi");
        assert_eq!(provider_product_label("deepseek"), "CodeWhale");
        assert_eq!(provider_product_label("codewhale"), "CodeWhale");
        assert_eq!(provider_product_label("copilot"), "Copilot");
        assert_eq!(provider_product_label("codebuddy"), "CodeBuddy");
    }
}
