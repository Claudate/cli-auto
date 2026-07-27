//! P2 budget tier ceiling + wave stickiness (pure).
//!
//! [INPUT]: spent/cap · PlanIR · committed (task_id, provider) · role tier
//! [OUTPUT]: optional max CostTier · sticky provider name · clamped tier
//! [POS]: domain/worker — used by cost_route + scheduler start
//! [PROTOCOL]: never force rewrite Explicit; ratios are product constants (not config)
//!   See docs/cost-aware-cli-router-2026-07-27.md §P2

use std::collections::{HashMap, HashSet};

use crate::domain::plan::PlanIR;

use super::cost_route::{
    default_cost_catalog, entry_tier, is_non_auto_provider, provider_tier, select_in_tier,
    CostPick, CostTier, ProviderCostEntry,
};

/// When run spend / cap ≥ this, new auto picks may not use Flagship.
pub const BUDGET_MID_RATIO: f64 = 0.70;
/// When run spend / cap ≥ this, new auto picks may only use Cheap.
pub const BUDGET_CHEAP_RATIO: f64 = 0.90;

/// Max tier allowed under current spend (None = no budget ceiling).
///
/// | spend/cap | ceiling |
/// |-----------|---------|
/// | no cap / &lt; mid | None |
/// | ≥ mid, &lt; cheap | Some(Mid) |
/// | ≥ cheap | Some(Cheap) |
pub fn budget_tier_ceiling(spent: f64, cap: Option<f64>) -> Option<CostTier> {
    budget_tier_ceiling_with_ratios(spent, cap, BUDGET_MID_RATIO, BUDGET_CHEAP_RATIO)
}

pub fn budget_tier_ceiling_with_ratios(
    spent: f64,
    cap: Option<f64>,
    mid_ratio: f64,
    cheap_ratio: f64,
) -> Option<CostTier> {
    let Some(cap) = cap.filter(|c| *c > 0.0 && c.is_finite()) else {
        return None;
    };
    if !spent.is_finite() || spent < 0.0 {
        return None;
    }
    let ratio = spent / cap;
    let cheap_r = cheap_ratio.max(mid_ratio);
    if ratio >= cheap_r {
        Some(CostTier::Cheap)
    } else if ratio >= mid_ratio {
        Some(CostTier::Mid)
    } else {
        None
    }
}

/// `min(requested, ceiling)` when ceiling is set.
pub fn clamp_tier(requested: CostTier, ceiling: Option<CostTier>) -> CostTier {
    match ceiling {
        None => requested,
        Some(max) if requested > max => max,
        Some(_) => requested,
    }
}

/// Dependency depth (wave index): roots = 0.
pub fn task_wave_depth(plan: &PlanIR, task_id: &str) -> usize {
    let index: HashMap<&str, &crate::domain::plan::TaskIR> =
        plan.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    if !index.contains_key(task_id) {
        return 0;
    }
    let mut memo: HashMap<String, usize> = HashMap::new();
    let mut visiting: HashSet<String> = HashSet::new();
    fn depth(
        id: &str,
        index: &HashMap<&str, &crate::domain::plan::TaskIR>,
        memo: &mut HashMap<String, usize>,
        visiting: &mut HashSet<String>,
    ) -> usize {
        if let Some(d) = memo.get(id) {
            return *d;
        }
        if !visiting.insert(id.to_string()) {
            return 0; // cycle guard
        }
        let d = match index.get(id) {
            None => 0,
            Some(t) if t.depends_on.is_empty() => 0,
            Some(t) => t
                .depends_on
                .iter()
                .map(|p| depth(p, index, memo, visiting).saturating_add(1))
                .max()
                .unwrap_or(0),
        };
        visiting.remove(id);
        memo.insert(id.to_string(), d);
        d
    }
    depth(task_id, &index, &mut memo, &mut visiting)
}

/// Tasks that should share a CLI with `task_id` when stickiness is on.
///
/// 1. Same non-empty `group`, else
/// 2. Same dependency wave depth.
pub fn sticky_cohort_ids(plan: &PlanIR, task_id: &str) -> Vec<String> {
    let Some(me) = plan.task(task_id) else {
        return vec![];
    };
    if let Some(g) = me.group.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return plan
            .tasks
            .iter()
            .filter(|t| {
                t.id != task_id
                    && t.group
                        .as_ref()
                        .map(|x| x.trim() == g)
                        .unwrap_or(false)
            })
            .map(|t| t.id.clone())
            .collect();
    }
    let my_wave = task_wave_depth(plan, task_id);
    plan.tasks
        .iter()
        .filter(|t| t.id != task_id && task_wave_depth(plan, &t.id) == my_wave)
        .map(|t| t.id.clone())
        .collect()
}

/// First committed provider from a cohort peer (stable plan order).
///
/// `committed`: (task_id, provider) already chosen this run / this routing pass.
///
/// When the cohort is **group-based**, any committed peer is fine (incl. explicit).
/// When the cohort is **wave-only** (no group), only `auto_committed` peers count —
/// otherwise an explicit gemini in the same DAG wave would hijack implement→codex.
pub fn sticky_provider(
    plan: &PlanIR,
    task_id: &str,
    committed: &[(String, String)],
    auto_committed: &[(String, String)],
) -> Option<String> {
    let Some(me) = plan.task(task_id) else {
        return None;
    };
    let has_group = me
        .group
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let source = if has_group {
        committed
    } else {
        auto_committed
    };
    let cohort: HashSet<String> = sticky_cohort_ids(plan, task_id).into_iter().collect();
    if cohort.is_empty() {
        return None;
    }
    for t in &plan.tasks {
        if !cohort.contains(&t.id) {
            continue;
        }
        if let Some((_, p)) = source.iter().find(|(id, _)| id == &t.id) {
            let p = p.trim();
            if !p.is_empty() && !is_non_auto_provider(p) {
                return Some(p.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Pick for one task: sticky peer if usable under tier, else cheapest in tier.
pub fn select_with_sticky(
    requested_tier: CostTier,
    ceiling: Option<CostTier>,
    sticky: Option<&str>,
    available: &[String],
    unhealthy: &[String],
    catalog: &[ProviderCostEntry],
) -> Option<CostPick> {
    let tier = clamp_tier(requested_tier, ceiling);
    if let Some(s) = sticky.map(str::trim).filter(|s| !s.is_empty()) {
        if !is_non_auto_provider(s) {
            let s_l = s.to_ascii_lowercase();
            let avail_ok = available.iter().any(|a| a.eq_ignore_ascii_case(&s_l));
            let bad = unhealthy.iter().any(|u| u.eq_ignore_ascii_case(&s_l));
            if avail_ok && !bad {
                if let Some(st) = provider_tier(&s_l).or_else(|| entry_tier(catalog, &s_l)) {
                    // Same band only — mid implement must not pull flagship inspect down.
                    if st == tier {
                        let cost_rank = catalog
                            .iter()
                            .find(|e| e.id == s_l.as_str())
                            .map(|e| e.cost_rank)
                            .unwrap_or(0);
                        return Some(CostPick {
                            provider: s_l,
                            tier: st,
                            requested_tier,
                            cost_rank,
                            borrowed_up: st > requested_tier,
                            sticky: true,
                            budget_clamped: ceiling.is_some() && requested_tier > tier,
                        });
                    }
                }
            }
        }
    }
    let mut pick = select_in_tier(tier, available, unhealthy, catalog)?;
    pick.requested_tier = requested_tier;
    // borrowed_up vs original request (not only vs clamped tier)
    pick.borrowed_up = pick.tier > requested_tier;
    pick.sticky = false;
    pick.budget_clamped = ceiling.is_some() && requested_tier > tier;
    Some(pick)
}

/// Whether mid-run budget downgrade may touch this provenance.
///
/// Explicit / tag / force / escalate stay. Soft + cost_auto may shrink.
pub fn may_budget_downgrade(route_source: Option<&str>) -> bool {
    match route_source.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") | Some("soft_fill") | Some("cost_auto") => true,
        Some("explicit")
        | Some("tag_routing")
        | Some("force")
        | Some("failover")
        | Some("cost_escalate") => false,
        // Unknown old → conservative allow only if looks soft-ish
        _ => false,
    }
}

/// Suggest a cheaper provider under ceiling; None if current already ok or no pick.
pub fn suggest_budget_downgrade(
    current_provider: &str,
    role_tier: CostTier,
    spent: f64,
    cap: Option<f64>,
    available: &[String],
    unhealthy: &[String],
) -> Option<CostPick> {
    let ceiling = budget_tier_ceiling(spent, cap)?;
    let cur_tier = provider_tier(current_provider).unwrap_or(CostTier::Flagship);
    if cur_tier <= ceiling {
        return None; // already within budget band
    }
    let want = clamp_tier(role_tier.min(cur_tier), Some(ceiling));
    let pick = select_in_tier(want, available, unhealthy, default_cost_catalog())?;
    if pick.provider.eq_ignore_ascii_case(current_provider) {
        return None;
    }
    Some(pick)
}

/// Topo-ish task id order: increasing wave depth, then plan order.
pub fn route_pass_order(plan: &PlanIR) -> Vec<String> {
    let mut ids: Vec<(usize, usize, String)> = plan
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (task_wave_depth(plan, &t.id), i, t.id.clone()))
        .collect();
    ids.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    ids.into_iter().map(|(_, _, id)| id).collect()
}

// --- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR, TaskRole};
    use crate::domain::worker::default_cost_catalog;
    use std::path::PathBuf;

    fn task(id: &str, deps: &[&str], group: Option<&str>, role: Option<TaskRole>) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            group: group.map(|s| s.into()),
            provider: "claude".into(),
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
            name: "p2".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
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
    fn ceiling_none_under_mid() {
        assert_eq!(budget_tier_ceiling(6.0, Some(10.0)), None);
        assert_eq!(budget_tier_ceiling(0.0, None), None);
    }

    #[test]
    fn ceiling_mid_then_cheap() {
        assert_eq!(
            budget_tier_ceiling(7.0, Some(10.0)),
            Some(CostTier::Mid)
        );
        assert_eq!(
            budget_tier_ceiling(9.0, Some(10.0)),
            Some(CostTier::Cheap)
        );
    }

    #[test]
    fn clamp_flagship_to_mid() {
        assert_eq!(
            clamp_tier(CostTier::Flagship, Some(CostTier::Mid)),
            CostTier::Mid
        );
        assert_eq!(clamp_tier(CostTier::Cheap, Some(CostTier::Mid)), CostTier::Cheap);
    }

    #[test]
    fn wave_depth_chain() {
        let ir = plan(vec![
            task("a", &[], None, None),
            task("b", &["a"], None, None),
            task("c", &["b"], None, None),
        ]);
        assert_eq!(task_wave_depth(&ir, "a"), 0);
        assert_eq!(task_wave_depth(&ir, "b"), 1);
        assert_eq!(task_wave_depth(&ir, "c"), 2);
    }

    #[test]
    fn sticky_same_group() {
        let ir = plan(vec![
            task("t1", &[], Some("ui"), Some(TaskRole::Implement)),
            task("t2", &[], Some("ui"), Some(TaskRole::Implement)),
            task("t3", &[], Some("api"), Some(TaskRole::Implement)),
        ]);
        let cohort = sticky_cohort_ids(&ir, "t1");
        assert!(cohort.contains(&"t2".into()));
        assert!(!cohort.contains(&"t3".into()));
        let sticky = sticky_provider(
            &ir,
            "t2",
            &[("t1".into(), "codex".into())],
            &[("t1".into(), "codex".into())],
        );
        assert_eq!(sticky.as_deref(), Some("codex"));
    }

    #[test]
    fn wave_sticky_ignores_explicit_seed() {
        let ir = plan(vec![
            task("impl", &[], None, Some(TaskRole::Implement)),
            task("kept", &[], None, None),
        ]);
        // Explicit gemini seeded in committed but not auto_committed.
        let sticky = sticky_provider(
            &ir,
            "impl",
            &[("kept".into(), "gemini".into())],
            &[],
        );
        assert!(sticky.is_none());
    }

    #[test]
    fn sticky_same_wave_without_group() {
        let ir = plan(vec![
            task("a", &[], None, None),
            task("b", &[], None, None),
            task("c", &["a"], None, None),
        ]);
        let cohort = sticky_cohort_ids(&ir, "a");
        assert!(cohort.contains(&"b".into()));
        assert!(!cohort.contains(&"c".into()));
    }

    #[test]
    fn select_prefers_sticky_when_in_tier() {
        let avail = vec!["codex".into(), "claude".into(), "qwen".into()];
        let pick = select_with_sticky(
            CostTier::Mid,
            None,
            Some("codex"),
            &avail,
            &[],
            default_cost_catalog(),
        )
        .unwrap();
        assert_eq!(pick.provider, "codex");
        assert!(pick.sticky);
    }

    #[test]
    fn suggest_downgrade_flagship_when_over_mid() {
        let avail = vec!["claude".into(), "codex".into()];
        let pick = suggest_budget_downgrade(
            "claude",
            CostTier::Flagship,
            8.0,
            Some(10.0),
            &avail,
            &[],
        )
        .unwrap();
        assert_eq!(pick.provider, "codex");
    }

    #[test]
    fn no_downgrade_when_already_cheap_enough() {
        let avail = vec!["codex".into(), "claude".into()];
        assert!(suggest_budget_downgrade(
            "codex",
            CostTier::Mid,
            8.0,
            Some(10.0),
            &avail,
            &[],
        )
        .is_none());
    }

    #[test]
    fn may_downgrade_sources() {
        assert!(may_budget_downgrade(Some("cost_auto")));
        assert!(may_budget_downgrade(Some("soft_fill")));
        assert!(!may_budget_downgrade(Some("explicit")));
        assert!(!may_budget_downgrade(Some("cost_escalate")));
    }
}
