//! Worker domain: pure route / capability / isolation / failover / cost-route policy (A1-4).
//!
//! **No** process spawn, **no** path layout for worktrees, **no** preflight IO.
//!
//! [INPUT]: provider name strings · plan fields · failover flags · available names
//! [OUTPUT]: fill decisions · isolation mode · failover/escalate target · cost route report
//! [POS]: domain/worker — scheduler + CLI call these; adapters do IO
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；soft-fill / cost-auto **不得**静默盖显式 route

mod cost_budget;
mod cost_route;
mod failover;
mod isolation;
mod route;
mod types;

pub use cost_budget::{
    budget_tier_ceiling, clamp_tier, may_budget_downgrade, route_pass_order, select_with_sticky,
    sticky_cohort_ids, sticky_provider, suggest_budget_downgrade, task_wave_depth, BUDGET_CHEAP_RATIO,
    BUDGET_MID_RATIO,
};
pub use cost_route::{
    apply_cost_aware_routing, apply_cost_aware_routing_with_catalog,
    apply_cost_aware_routing_with_opts, catalog_provider_ids, default_cost_catalog, entry_tier,
    filter_auto_available, is_non_auto_provider, next_escalate_target, provider_tier,
    role_default_tier, select_in_tier, CostPick, CostRouteChange, CostRouteOpts, CostRouteReport,
    CostTier, ProviderCostEntry,
};
pub use failover::FailoverPolicy;
pub use isolation::{is_multi_provider, isolation_on_fail, IsolationOnFail};
pub use route::{
    apply_route_fill, apply_worker_defaults, is_still_default_route, RouteFillMode, RouteFillReport,
};
pub use types::{CapabilityFlags, ProviderId, WorkerRoute};
