//! Route stamping options (P0 cost auto, P1-2 provenance).
//!
//! [PROTOCOL]: 变更时更新 app/CLAUDE.md · materialize/mod.rs

#[derive(Debug, Clone, Default)]
pub struct MaterializeRouteOpts {
    /// When true, skip cost-aware rewrite (CLI provider override is last write).
    pub skip_cost_route: bool,
}

pub fn apply_materialize_route_opts(
    ir: &mut crate::plan::PlanIR,
    opts: &MaterializeRouteOpts,
    route_report: Option<&crate::domain::worker::RouteFillReport>,
) -> Option<String> {
    // 简化版：原逻辑已在 materialize_run_with_route_opts
    if opts.skip_cost_route {
        return None;
    }
    // 原逻辑略
    None
}
