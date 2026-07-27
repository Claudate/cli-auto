//! CLI `cco run` — thin presentation; open-run via app (A5-1).
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；Mode B → `app::split::confirm_materialize`；
//!   ParseOnly → `app::run::materialize_run`（返回 drop optional 后的 IR · D-T3-1）；
//!   soft-fill → `app::run::apply_provider_override`；
//!   loop → `app::run::prepare_scheduler`
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md · **禁止**手搓 second soft-fill / confirm

use std::path::PathBuf;

use anyhow::Result;

use crate::app::run as run_uc;
use crate::app::split as split_uc;
use crate::cli::commands::common::{
    plan_then_load_ir, resolve_term_kind, run_scheduler_loop,
};
use crate::cli::interactive;
use crate::cli::TermKindArg;
use crate::config::Config;
use crate::graph::format_graph;
use crate::plan::{is_structured_adapter, load_plan, peek_adapter};

pub async fn run(
    config: &Config,
    project: Option<PathBuf>,
    plan: Option<PathBuf>,
    yes: bool,
    skip_plan: bool,
    plan_mode: String,
    mode: Option<String>,
    provider: Option<String>,
    force_provider: Option<String>,
    max_parallel: Option<usize>,
    adapter: Option<String>,
    mirror_state: bool,
    from_task: Option<String>,
    only: Option<Vec<String>>,
    dry_run: bool,
    tui: bool,
    auto_open_terminal: bool,
    terminal_kind: Option<TermKindArg>,
    max_budget: Option<f64>,
    effort: Option<String>,
) -> Result<i32> {
    // Session-level effort override (config clone so we don't mutate shared disk state).
    let mut config = config.clone();
    if let Some(raw) = effort.as_deref() {
        if let Some(n) = crate::config::normalize_effort(raw) {
            config.default.effort = n;
            println!("effort: {}", config.default.effort);
        } else {
            eprintln!(
                "warning: unknown --effort {raw:?}; expected low|medium|high|xhigh|max|ultracode (ignored)"
            );
        }
    }
    let config = &config;
    let project = interactive::resolve_project(project, true)?;
    interactive::ensure_project_dir(&project)?;
    let plan = interactive::resolve_plan(&project, plan, true)?;

    // P0-1/P0-2: structured → auto skip-plan; prose → plan job then confirm
    let adapter_name = if let Some(a) = adapter.as_deref() {
        a.to_string()
    } else {
        peek_adapter(&project, &plan).unwrap_or_else(|_| "raw-single".into())
    };
    let auto_skip = is_structured_adapter(&adapter_name);
    let do_skip = skip_plan || auto_skip || adapter.is_some();

    // Planner still receives soft --provider as a hint; force-provider is run-level only.
    let planner_provider = force_provider.clone().or_else(|| provider.clone());

    let plan_job_id = if do_skip {
        if auto_skip && !skip_plan {
            println!("skip-plan: structured plan (adapter={adapter_name}) → direct exec");
        } else if skip_plan {
            println!("skip-plan: forced (--skip-plan)");
        }
        None
    } else {
        println!(
            "planning… (adapter={adapter_name} is not structured; Mode B plan job)"
        );
        let (preview_ir, job_id) = plan_then_load_ir(
            config,
            &project,
            &plan,
            &plan_mode,
            planner_provider,
            mode.clone(),
            max_parallel,
        )?;
        // Preview graph (proposed; optionals still visible until confirm).
        let mut preview = preview_ir;
        if let Some(report) =
            run_uc::apply_provider_override(&mut preview, provider.clone(), force_provider.clone())
        {
            println!("{}", report.summary_line());
        }
        apply_mode_parallel(&mut preview, mode.as_ref(), max_parallel);
        print!("{}", format_graph(&preview));
        Some(job_id)
    };

    // ParseOnly: load + soft-fill for display before TTY confirm.
    // Keep the fill report so materialize can stamp route_source (P1-2).
    let mut parse_only = if do_skip {
        let mut ir = load_plan(&project, &plan, adapter.as_deref(), config)?;
        let report =
            run_uc::apply_provider_override(&mut ir, provider.clone(), force_provider.clone());
        if let Some(ref r) = report {
            println!("{}", r.summary_line());
        }
        apply_mode_parallel(&mut ir, mode.as_ref(), max_parallel);
        print!("{}", format_graph(&ir));
        Some((ir, report))
    } else {
        None
    };

    if !yes && !dry_run {
        if !interactive::confirm("proceed?", false)? {
            println!("aborted");
            return Ok(1);
        }
    }

    // ── Open-run (app only; no hand-rolled new_run_id / mark_confirmed) ──
    let (run_id, run_state, ir, route_line) = if let Some(job_id) = plan_job_id {
        let patches = split_uc::ConfirmPatches {
            provider,
            force_provider,
            mode,
            max_parallel,
            // Session effort already on config.default; also force onto tasks at open-run.
            effort: Some(config.default.effort.clone()),
        };
        // Sole Mode B open-run (optional drop + soft defaults + materialize + mark).
        let (run_id, st, ir, route_line) =
            split_uc::confirm_materialize(config, &job_id, patches)?;
        (run_id, st, ir, route_line)
    } else {
        let (ir, report) = parse_only
            .take()
            .expect("ParseOnly path always loads IR before confirm");
        // Documented ParseOnly — not Mode B; still drops optional && !include (A0-R4).
        // Use **returned** IR for the scheduler (D-T3-1). Stamp route_source from report.
        let (run_id, st, ir, cost_line) = run_uc::materialize_run_with_route(
            config,
            project.clone(),
            &ir,
            report.as_ref(),
        )?;
        (run_id, st, ir, cost_line)
    };

    println!("run_id: {run_id}");
    println!("run_dir: {}", run_state.run_dir.display());
    if let Some(line) = route_line {
        println!("{line}");
    }

    let mirror = if mirror_state || config.default.mirror_state {
        let m = project.join(".cco").join("runs");
        std::fs::create_dir_all(&m)?;
        Some(m)
    } else {
        None
    };

    let term_kind = resolve_term_kind(terminal_kind, config);
    let auto_open = auto_open_terminal || config.terminal.auto_open_on_start;
    let opts = run_uc::ForegroundOpts {
        max_parallel,
        yes,
        only: only.map(|v| v.into_iter().collect()),
        from_task,
        dry_run,
        mirror_state: mirror,
        auto_open_terminal: auto_open,
        terminal_kind: term_kind,
        max_budget,
    };
    let sched = run_uc::prepare_scheduler(config, ir, run_state, opts)?;
    run_uc::preflight_plan(&sched.registry, &sched.plan).await?;
    run_scheduler_loop(sched, config, &run_id, tui).await
}

fn apply_mode_parallel(
    ir: &mut crate::plan::PlanIR,
    mode: Option<&String>,
    max_parallel: Option<usize>,
) {
    if let Some(m) = mode {
        for t in &mut ir.tasks {
            t.mode = m.clone();
        }
        ir.default_mode = m.clone();
    }
    if let Some(mp) = max_parallel {
        ir.max_parallel = mp;
    }
}
