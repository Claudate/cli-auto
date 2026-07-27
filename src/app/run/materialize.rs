//! Run disk materialization (A5-1 · S-run extract).
//!
//! [INPUT]: Config · project · PlanIR / plan path + adapter · optional RouteFillReport
//! [OUTPUT]: (run_id, RunState, PlanIR) · ParseOnly also loads then materializes
//! [POS]: app::run sub-module; does **not** spawn scheduler
//! [PROTOCOL]: Mode B IR only via split::confirm_materialize; ParseOnly is documented
//!   non–Mode B **but still** drops `optional && !include` (A0-R4 · D-T3-1) — same
//!   `materialize_selected_tasks` as confirm. Callers **must** schedule the returned IR.
//!   Stamps `route_source` here (P1-2) — domain never writes paths / RunState.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::domain::plan::{
    assign_closeout_owners, build_host_checklist, format_checklist_for_prompt, inject_closeout_task,
    SYS_CLOSEOUT_ID,
};
use crate::domain::worker::{
    apply_cost_aware_routing_with_opts, CostRouteOpts, CostRouteReport, RouteFillReport,
};
use crate::plan::{load_plan, materialize_selected_tasks, PlanIR};
use crate::state::{self, RunState};

use super::provenance::{stamp_cost_route, stamp_route_fill, stamp_route_inferred};
use super::route::list_cost_route_available;

/// Disk materialization of a new run (run_id + run.json + plan.resolved.json).
///
/// Always runs [`materialize_selected_tasks`] so unselected optionals never land
/// in `run.json` / `plan.resolved.json` / the returned IR (A0-R4 · D-T3-1).
/// Does **not** spawn the scheduler.
///
/// Mode B callers obtain `ir` via [`crate::app::split::confirm_materialize`]
/// (which may already have dropped optionals — re-apply is idempotent).
/// ParseOnly / `--skip-plan` use this after soft-fill, or [`materialize_parse_only`].
///
/// When `route_report` is `Some`, stamps each task's `route_source` from the report
/// (soft filled / kept explicit / force). When `None`, infers from the final IR.
///
/// Cost-aware routing (P0) runs **after** soft/tag fill unless
/// `skip_cost_route` (CLI `--provider` / `--force-provider` last-write).
///
/// Returns `(run_id, state, ir)` — **use the returned `ir` for `prepare_scheduler`**.
pub fn materialize_run(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
) -> Result<(String, RunState, PlanIR)> {
    materialize_run_with_route(config, project, ir, None)
}

/// Same as [`materialize_run`] but applies an optional last-write fill report for
/// P1-2 `route_source` provenance before the first `run.json` save.
pub fn materialize_run_with_route(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
    route_report: Option<&RouteFillReport>,
) -> Result<(String, RunState, PlanIR)> {
    materialize_run_with_route_opts(
        config,
        project,
        ir,
        route_report,
        MaterializeRouteOpts::default(),
    )
}

/// Options for materialize route stamping (P0 cost auto).
#[derive(Debug, Clone, Default)]
pub struct MaterializeRouteOpts {
    /// When true, skip cost-aware rewrite (CLI provider override is last write).
    pub skip_cost_route: bool,
}

/// Materialize with explicit cost-route control.
pub fn materialize_run_with_route_opts(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
    route_report: Option<&RouteFillReport>,
    opts: MaterializeRouteOpts,
) -> Result<(String, RunState, PlanIR)> {
    if !project.is_dir() {
        bail!("项目路径不是目录: {}", project.display());
    }
    // Drop optional && !include before any disk write or scheduler handoff.
    let mut ir = materialize_selected_tasks(ir.clone())?;
    // Ensure E1/E2: host checklist + optional sys-closeout before soft-fill.
    let plan_md = std::fs::read_to_string(&ir.source_path).ok();
    let mut checklist = build_host_checklist(&ir, plan_md.as_deref());
    let paste = format_checklist_for_prompt(&checklist.items);
    inject_closeout_task(&mut ir, config.default.auto_closeout, Some(&paste));
    if ir.tasks.iter().any(|t| t.id == SYS_CLOSEOUT_ID) {
        assign_closeout_owners(&mut checklist, SYS_CLOSEOUT_ID);
    }
    // Soft-fill Claude effort from config when task opts lack it (does not overwrite explicit).
    apply_effort(&mut ir, config, None);
    // Soft-fill permission_mode so unattended workers can Edit/Bash (default bypass).
    apply_permission_mode(&mut ir, config, None);

    // P0–P3: role→tier→cheapest (+ sticky · budget · optional intent).
    let cost_report = if opts.skip_cost_route || !config.default.cost_route_enabled {
        CostRouteReport::default()
    } else {
        let available = list_cost_route_available(config);
        apply_cost_aware_routing_with_opts(
            &mut ir,
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
    };

    ir.validate()
        .with_context(|| "validate after ensure closeout inject")?;
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let project = project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", project.display()))?;
    let mut run_state = RunState::new(run_id.clone(), project, &ir, run_dir.clone());
    // P1-2: stamp provenance at RunState assembly (never in domain).
    if let Some(report) = route_report {
        stamp_route_fill(&mut run_state, &ir, report);
    }
    stamp_route_inferred(&mut run_state, &ir);
    // Cost auto last among open-run stamps (overrides soft_fill on rewritten ids).
    if !cost_report.changed.is_empty() {
        stamp_cost_route(&mut run_state, &cost_report);
        if let Some(line) = cost_report.summary_line() {
            let _ = run_state.event(
                "cost_route",
                serde_json::json!({
                    "summary": line,
                    "changed": cost_report.changed.len(),
                }),
            );
        }
    }
    run_state.save()?;
    let resolved = run_dir.join("plan.resolved.json");
    std::fs::write(&resolved, serde_json::to_string_pretty(&ir)?)?;
    // Host checklist for E1/E3 prompts and report compare.
    let checklist_path = run_dir.join("plan.checklist.json");
    if let Ok(body) = serde_json::to_string_pretty(&checklist) {
        let _ = std::fs::write(&checklist_path, body);
    }
    Ok((run_id, run_state, ir))
}

/// Apply Claude reasoning effort onto task `provider_opts`.
///
/// - `override_effort` set → force all claude/fake tasks to that level (UI/CLI pick).
/// - else soft-fill from `config.default.effort` only when the key is missing/empty.
pub fn apply_effort(ir: &mut PlanIR, config: &Config, override_effort: Option<&str>) {
    let force = override_effort
        .and_then(crate::config::normalize_effort);
    let soft = crate::config::normalize_effort(&config.default.effort)
        .unwrap_or_else(|| "high".into());
    for t in &mut ir.tasks {
        let p = t.provider.to_ascii_lowercase();
        if p != "claude" && p != "fake" {
            continue;
        }
        if !t.provider_opts.is_object() {
            t.provider_opts = serde_json::json!({});
        }
        if let Some(ref e) = force {
            t.provider_opts["effort"] = serde_json::json!(e);
            continue;
        }
        let missing = t
            .provider_opts
            .get("effort")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if missing {
            t.provider_opts["effort"] = serde_json::json!(soft);
        }
    }
}

/// Apply Claude `permission_mode` onto task `provider_opts`.
///
/// Unattended workers have no permission UI. Soft-fill defaults to
/// `bypassPermissions` (from config) so Edit/Bash are not auto-denied.
/// Explicit per-task values are kept; `override_mode` forces all claude/fake tasks.
pub fn apply_permission_mode(ir: &mut PlanIR, config: &Config, override_mode: Option<&str>) {
    let force = override_mode.and_then(crate::config::normalize_permission_mode);
    let soft = crate::config::normalize_permission_mode(&config.default.permission_mode)
        .unwrap_or_else(|| "bypassPermissions".into());
    for t in &mut ir.tasks {
        let p = t.provider.to_ascii_lowercase();
        if p != "claude" && p != "fake" {
            continue;
        }
        if !t.provider_opts.is_object() {
            t.provider_opts = serde_json::json!({});
        }
        if let Some(ref m) = force {
            t.provider_opts["permission_mode"] = serde_json::json!(m);
            continue;
        }
        let missing = t
            .provider_opts
            .get("permission_mode")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if missing {
            t.provider_opts["permission_mode"] = serde_json::json!(soft);
        }
    }
}

/// ParseOnly / structured / `--skip-plan`: load plan from disk and materialize.
///
/// **Not** Mode B. Documented ParseOnly path — does not create a plan job.
/// Soft-fill / force-provider must be applied by the caller via
/// [`super::apply_provider_override`] before this call (or pass already-patched IR via
/// [`materialize_run_with_route`] after load). Unselected optionals are still dropped (A0-R4).
pub fn materialize_parse_only(
    config: &Config,
    project: PathBuf,
    plan: &Path,
    adapter: Option<&str>,
) -> Result<(String, RunState, PlanIR)> {
    let ir = load_plan(&project, plan, adapter, config)?;
    materialize_run(config, project, &ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR};
    use crate::plan::PlanIR;

    fn test_cfg(root: &std::path::Path) -> Config {
        let mut c = Config::default();
        c.state_root = root.to_path_buf();
        c
    }

    #[test]
    fn apply_permission_mode_soft_fills_bypass_default() {
        let cfg = Config::default();
        assert_eq!(cfg.default.permission_mode, "bypassPermissions");
        let mut ir = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "p".into(),
            adapter: "test".into(),
            source_path: std::path::PathBuf::from("p.md"),
            max_parallel: 1,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![TaskIR {
                id: "t1".into(),
                title: "t".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "x".into(),
                verify_cmd: None,
                acceptance: None,
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: serde_json::json!({}),
                optional: false,
                include: true,
                role: None,
                scope: None,
                outputs: vec![],
                tags: vec![],
            }],
        };
        apply_permission_mode(&mut ir, &cfg, None);
        assert_eq!(
            ir.tasks[0].provider_opts["permission_mode"].as_str(),
            Some("bypassPermissions")
        );
        // explicit kept
        ir.tasks[0].provider_opts["permission_mode"] = serde_json::json!("dontAsk");
        apply_permission_mode(&mut ir, &cfg, None);
        assert_eq!(
            ir.tasks[0].provider_opts["permission_mode"].as_str(),
            Some("dontAsk")
        );
        // force override
        apply_permission_mode(&mut ir, &cfg, Some("bypass"));
        assert_eq!(
            ir.tasks[0].provider_opts["permission_mode"].as_str(),
            Some("bypassPermissions")
        );
    }

    fn sample_ir(project: &std::path::Path) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "opt-parseonly".into(),
            adapter: "cco-plan/v1".into(),
            source_path: project.join("docs/plans/opt.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                TaskIR {
                    id: "must".into(),
                    title: "必做".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "must\nCCO_DONE ok".into(),
                    verify_cmd: None,
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: false,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
                TaskIR {
                    id: "maybe".into(),
                    title: "可选（未勾选）".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "maybe\nCCO_DONE ok".into(),
                    verify_cmd: None,
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: true,
                    include: false,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
                TaskIR {
                    id: "maybe_on".into(),
                    title: "可选（已勾选）".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "maybe_on\nCCO_DONE ok".into(),
                    verify_cmd: None,
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: true,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
            ],
        }
    }

    /// D-T3-1 / A0-R4: ParseOnly materialize drops unselected optionals from disk + return IR.
    #[test]
    fn materialize_run_drops_unselected_optional() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("docs/plans")).unwrap();
        let cfg = test_cfg(tmp.path());
        let ir = sample_ir(&project);

        let (run_id, st, out) = materialize_run(&cfg, project, &ir).unwrap();
        assert!(!run_id.is_empty());
        assert!(out.task("must").is_some());
        assert!(out.task("maybe_on").is_some());
        assert!(
            out.task("maybe").is_none(),
            "unselected optional must not appear in returned IR"
        );
        assert!(!st.tasks.contains_key("maybe"));
        assert!(st.tasks.contains_key("must"));
        assert!(st.tasks.contains_key("maybe_on"));

        let resolved: PlanIR = serde_json::from_str(
            &std::fs::read_to_string(st.run_dir.join("plan.resolved.json")).unwrap(),
        )
        .unwrap();
        assert!(resolved.task("maybe").is_none());
        assert_eq!(resolved.tasks.len(), 2);
        // P1-2: new runs stamp route_source (inferred soft_fill when provider==default).
        assert!(st.tasks["must"].route_source.is_some());
        assert!(st.tasks["maybe_on"].route_source.is_some());
    }

    /// P1-2: soft fill report → filled soft_fill, kept explicit in run.json.
    #[test]
    fn materialize_stamps_soft_fill_provenance() {
        use crate::domain::worker::{apply_route_fill, RouteFillMode};
        use crate::state::RouteSource;

        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("docs/plans")).unwrap();
        let mut cfg = test_cfg(tmp.path());
        // Isolate soft-fill stamp contract from P0 cost auto.
        cfg.default.cost_route_enabled = false;
        let mut ir = sample_ir(&project);
        // Multi-provider soft-fill needs worktree (validate gate).
        ir.worktree = true;
        ir.tasks[0].worktree = Some(true);
        ir.tasks[2].worktree = Some(true);
        // Make must explicit codex, leave others on default fake.
        ir.tasks[0].provider = "codex".into();
        let report = apply_route_fill(&mut ir, "fake", RouteFillMode::Soft).unwrap();
        let (_run_id, st, _out) =
            materialize_run_with_route(&cfg, project, &ir, Some(&report)).unwrap();
        assert_eq!(st.tasks["must"].route_source, Some(RouteSource::Explicit));
        assert_eq!(
            st.tasks["maybe_on"].route_source,
            Some(RouteSource::SoftFill)
        );
    }
}
