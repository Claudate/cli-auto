//! Provider route override + cost-aware available list (A0-R3 / A1-4 · S-run extract).
//!
//! [INPUT]: PlanIR · optional --provider / --force-provider · Config for cost pool
//! [OUTPUT]: RouteFillReport when applied · installed auto-eligible provider names
//! [POS]: app::run sub-module; policy pure in domain/worker::apply_route_fill / cost_route
//! [PROTOCOL]: soft-fill must not overwrite explicit routes; force wipe only with force flag;
//!   cost auto never runs after CLI provider override (last-write)

use crate::config::Config;
use crate::domain::worker::{
    apply_cost_aware_routing_with_opts, apply_route_fill, catalog_provider_ids,
    filter_auto_available, CostRouteOpts, CostRouteReport, RouteFillMode, RouteFillReport,
};
use crate::plan::PlanIR;
use crate::runtime::provider::resolve_bin_on_disk;

/// CLI `cco run` provider override: soft-fill vs force wipe (A0-R3 / A1-4).
///
/// | flag | behavior |
/// |------|----------|
/// | none | no change |
/// | `--provider P` | soft-fill via [`RouteFillMode::Soft`] |
/// | `--force-provider P` | force wipe via [`RouteFillMode::Force`] |
///
/// When both are set, force wins. Returns the domain [`RouteFillReport`] when applied
/// (callers stamp `route_source` via [`super::provenance::stamp_route_fill`]).
pub fn apply_provider_override(
    ir: &mut PlanIR,
    provider: Option<String>,
    force_provider: Option<String>,
) -> Option<RouteFillReport> {
    if let Some(p) = force_provider {
        return apply_route_fill(ir, &p, RouteFillMode::Force);
    }
    if let Some(p) = provider {
        return apply_route_fill(ir, &p, RouteFillMode::Soft);
    }
    None
}

/// Enabled + bin-resolvable production CLIs for cost-aware routing (no preflight spawn).
///
/// Uses config `providers.*.enabled` and [`resolve_bin_on_disk`]. fake/sdk filtered out.
/// Always includes `default_provider` when enabled so a lone-flagship install still routes.
pub fn list_cost_route_available(config: &Config) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for id in catalog_provider_ids() {
        let Some(pc) = config.provider(id) else {
            continue;
        };
        if !pc.enabled {
            continue;
        }
        if resolve_bin_on_disk(&pc.bin).is_some() {
            names.push(id.to_string());
        }
    }
    // Ensure plan default is considered when its bin resolves (or is "claude" common path).
    let def = config.default.default_provider.trim();
    if !def.is_empty()
        && !names.iter().any(|n| n.eq_ignore_ascii_case(def))
        && config
            .provider(def)
            .map(|pc| pc.enabled && resolve_bin_on_disk(&pc.bin).is_some())
            .unwrap_or(false)
    {
        names.push(def.to_ascii_lowercase());
    }
    filter_auto_available(names)
}

/// Dry-run cost routing on a **clone** of the plan (confirm desk preview).
///
/// Does not mutate the caller's IR. Empty report when cost route is off.
pub fn preview_cost_route(config: &Config, ir: &PlanIR) -> CostRouteReport {
    if !config.default.cost_route_enabled {
        return CostRouteReport::default();
    }
    let mut clone = ir.clone();
    let available = list_cost_route_available(config);
    apply_cost_aware_routing_with_opts(
        &mut clone,
        &available,
        &[],
        CostRouteOpts {
            enabled: true,
            spent_usd: 0.0,
            budget_cap_usd: config.default.run_max_budget_usd,
            sticky: true,
            intent: config.default.cost_intent_enabled,
        },
        crate::domain::worker::default_cost_catalog(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR, TaskRole};
    use std::path::PathBuf;

    fn sample_ir() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "preview".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![TaskIR {
                id: "impl".into(),
                title: "实现".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "do work".into(),
                verify_cmd: None,
                acceptance: None,
                timeout_secs: None,
                worktree: None,
                provider_opts: serde_json::json!({}),
                optional: false,
                include: true,
                role: Some(TaskRole::Implement),
                scope: None,
                outputs: vec![],
                tags: vec![],
            }],
        }
    }

    #[test]
    fn preview_off_when_disabled() {
        let mut cfg = Config::default();
        cfg.default.cost_route_enabled = false;
        let r = preview_cost_route(&cfg, &sample_ir());
        assert!(r.changed.is_empty());
        assert!(r.summary_line().is_none());
    }
}
