//! Apply cost-aware routing onto PlanIR (P0–P3 orchestration).
//!
//! [INPUT]: PlanIR · available · CostRouteOpts · catalog
//! [OUTPUT]: CostRouteReport (mutates still-default providers)
//! [POS]: domain/worker — pure; materialize calls this
//! [PROTOCOL]: never rewrite explicit / tag-kept; fake/sdk never auto

use crate::domain::plan::PlanIR;

use super::cost_budget::{
    budget_tier_ceiling, clamp_tier, route_pass_order, select_with_sticky, sticky_provider,
};
use super::cost_intent::effective_tier;
use super::cost_route::{
    default_cost_catalog, role_default_tier, CostRouteChange, CostRouteReport, ProviderCostEntry,
};
use super::route::is_still_default_route;

/// Open-run / materialize options for cost routing (P0–P3).
#[derive(Debug, Clone)]
pub struct CostRouteOpts {
    pub enabled: bool,
    /// Run spend so far (0 at first materialize).
    pub spent_usd: f64,
    /// `run_max_budget_usd` when set.
    pub budget_cap_usd: Option<f64>,
    /// P2: same group / wave reuses prior pick (default true).
    pub sticky: bool,
    /// P3: heuristic intent nudge on title/prompt/tags (default **false**).
    pub intent: bool,
}

impl Default for CostRouteOpts {
    fn default() -> Self {
        Self {
            enabled: true,
            spent_usd: 0.0,
            budget_cap_usd: None,
            sticky: true,
            intent: false,
        }
    }
}

/// Rewrite still-default task providers by role→tier→cheapest available.
///
/// No-op when `enabled` is false. Does not touch explicit / tag-kept engines.
pub fn apply_cost_aware_routing(
    plan: &mut PlanIR,
    available: &[String],
    unhealthy: &[String],
    enabled: bool,
) -> CostRouteReport {
    apply_cost_aware_routing_with_opts(
        plan,
        available,
        unhealthy,
        CostRouteOpts {
            enabled,
            ..CostRouteOpts::default()
        },
        default_cost_catalog(),
    )
}

pub fn apply_cost_aware_routing_with_catalog(
    plan: &mut PlanIR,
    available: &[String],
    unhealthy: &[String],
    enabled: bool,
    catalog: &[ProviderCostEntry],
) -> CostRouteReport {
    apply_cost_aware_routing_with_opts(
        plan,
        available,
        unhealthy,
        CostRouteOpts {
            enabled,
            ..CostRouteOpts::default()
        },
        catalog,
    )
}

/// Full P0–P3 pass: wave order · sticky · budget ceiling · optional intent.
pub fn apply_cost_aware_routing_with_opts(
    plan: &mut PlanIR,
    available: &[String],
    unhealthy: &[String],
    opts: CostRouteOpts,
    catalog: &[ProviderCostEntry],
) -> CostRouteReport {
    let mut report = CostRouteReport::default();
    if !opts.enabled {
        return report;
    }
    let ceiling = budget_tier_ceiling(opts.spent_usd, opts.budget_cap_usd);
    let default = plan.default_provider.clone();
    let order = route_pass_order(plan);
    let mut committed: Vec<(String, String)> = Vec::new();
    // Only engines chosen by this auto pass (wave sticky ignores bare explicit seeds).
    let mut auto_committed: Vec<(String, String)> = Vec::new();

    // Seed committed with already-explicit engines so **group** sticky can follow them.
    for t in &plan.tasks {
        if !is_still_default_route(&t.provider, &default) {
            committed.push((t.id.clone(), t.provider.clone()));
        }
    }

    for id in order {
        let Some(idx) = plan.tasks.iter().position(|t| t.id == id) else {
            continue;
        };
        let (provider, role, tags, title, prompt) = {
            let t = &plan.tasks[idx];
            (
                t.provider.clone(),
                t.role,
                t.tags.clone(),
                t.title.clone(),
                t.prompt.clone(),
            )
        };
        if !is_still_default_route(&provider, &default) {
            continue;
        }
        if let Some(implied) = crate::domain::plan::tag_implied_provider(&tags) {
            if provider.eq_ignore_ascii_case(implied) {
                committed.push((id.clone(), provider));
                continue;
            }
        }
        let role_tier = role_default_tier(role);
        let (want_tier, intent_note) =
            effective_tier(opts.intent, role, role_tier, &title, &prompt, &tags);
        let requested = clamp_tier(want_tier, ceiling);
        let sticky = if opts.sticky {
            sticky_provider(plan, &id, &committed, &auto_committed)
        } else {
            None
        };
        let Some(mut pick) = select_with_sticky(
            want_tier,
            ceiling,
            sticky.as_deref(),
            available,
            unhealthy,
            catalog,
        ) else {
            report.skipped_ids.push(id);
            continue;
        };
        pick.budget_clamped = ceiling.is_some() && want_tier > requested;
        let from = provider;
        let to = pick.provider.clone();
        let tier = pick.tier;
        let mut rationale = pick.rationale_zh();
        if let Some(note) = intent_note {
            rationale = format!("{rationale}·{note}");
        }
        if !from.eq_ignore_ascii_case(&to) {
            plan.tasks[idx].provider = to.clone();
        }
        report.changed.push(CostRouteChange {
            task_id: id.clone(),
            from,
            to: to.clone(),
            tier,
            rationale,
        });
        committed.push((id.clone(), to.clone()));
        auto_committed.push((id, to));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR, TaskRole};
    use crate::domain::worker::CostTier;
    use std::path::PathBuf;

    fn task(id: &str, provider: &str, role: Option<TaskRole>) -> TaskIR {
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
            role,
            scope: None,
            outputs: vec![],
            tags: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "cost".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("c.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn apply_rewrites_default_implement_to_codex() {
        let mut ir = plan(vec![
            task("impl", "claude", Some(TaskRole::Implement)),
            task("insp", "claude", Some(TaskRole::Inspect)),
            task("kept", "gemini", None),
        ]);
        let avail = vec!["claude".into(), "codex".into(), "gemini".into()];
        let r = apply_cost_aware_routing(&mut ir, &avail, &[], true);
        assert_eq!(ir.tasks[0].provider, "codex");
        assert_eq!(ir.tasks[1].provider, "claude");
        assert_eq!(ir.tasks[2].provider, "gemini");
        assert!(r
            .changed
            .iter()
            .any(|c| c.task_id == "impl" && c.to == "codex"));
        assert!(r.summary_line().unwrap().contains("Codex"));
    }

    #[test]
    fn disabled_is_noop() {
        let mut ir = plan(vec![task("impl", "claude", Some(TaskRole::Implement))]);
        let avail = vec!["codex".into(), "claude".into()];
        let r = apply_cost_aware_routing(&mut ir, &avail, &[], false);
        assert!(r.changed.is_empty());
        assert_eq!(ir.tasks[0].provider, "claude");
    }

    #[test]
    fn tag_kept_not_overridden() {
        let mut t = task("t", "codex", Some(TaskRole::Implement));
        t.tags = vec!["codex".into()];
        let mut ir = plan(vec![t]);
        let avail = vec!["claude".into(), "codex".into()];
        let r = apply_cost_aware_routing(&mut ir, &avail, &[], true);
        assert_eq!(ir.tasks[0].provider, "codex");
        assert!(r.changed.is_empty());
    }

    #[test]
    fn intent_trivial_drops_implement_to_cheap() {
        let mut t = task("typo", "claude", Some(TaskRole::Implement));
        t.title = "fix typo in README".into();
        t.prompt = "错别字 only, no code change".into();
        let mut ir = plan(vec![t]);
        let avail = vec![
            "claude".into(),
            "codex".into(),
            "qwen".into(),
            "gemini".into(),
        ];
        let r = apply_cost_aware_routing_with_opts(
            &mut ir,
            &avail,
            &[],
            CostRouteOpts {
                enabled: true,
                intent: true,
                ..CostRouteOpts::default()
            },
            default_cost_catalog(),
        );
        assert_eq!(ir.tasks[0].provider, "qwen"); // cheapest in cheap pool among avail
        assert!(r.changed[0].rationale.contains("偏简"));
    }

    #[test]
    fn intent_hard_bumps_implement_to_flagship() {
        let mut t = task("arch", "claude", Some(TaskRole::Implement));
        t.title = "redesign architecture".into();
        t.prompt = "跨模块重构 auth and session".into();
        let mut ir = plan(vec![t]);
        let avail = vec!["claude".into(), "codex".into()];
        let r = apply_cost_aware_routing_with_opts(
            &mut ir,
            &avail,
            &[],
            CostRouteOpts {
                enabled: true,
                intent: true,
                ..CostRouteOpts::default()
            },
            default_cost_catalog(),
        );
        assert_eq!(ir.tasks[0].provider, "claude");
        assert_eq!(r.changed[0].tier, CostTier::Flagship);
        assert!(r.changed[0].rationale.contains("偏难"));
    }

    #[test]
    fn intent_off_keeps_mid_on_typo_title() {
        let mut t = task("typo", "claude", Some(TaskRole::Implement));
        t.title = "fix typo".into();
        t.prompt = "typo only".into();
        let mut ir = plan(vec![t]);
        let avail = vec!["claude".into(), "codex".into(), "qwen".into()];
        apply_cost_aware_routing_with_opts(
            &mut ir,
            &avail,
            &[],
            CostRouteOpts {
                enabled: true,
                intent: false,
                ..CostRouteOpts::default()
            },
            default_cost_catalog(),
        );
        assert_eq!(ir.tasks[0].provider, "codex"); // mid default
    }
}
