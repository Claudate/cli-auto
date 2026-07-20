//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;

use anyhow::Result;

use crate::cli::commands::common::{
    make_terminal_manager, plan_then_load_ir, poll_interval, preflight_providers,
    provider_parallel_caps, resolve_term_kind, run_scheduler,
};
use crate::cli::interactive;
use crate::cli::TermKindArg;
use crate::config::Config;
use crate::graph::format_graph;
use crate::plan::{is_structured_adapter, load_plan, peek_adapter, PlanIR};
use crate::runtime::provider::ProviderRegistry;
use crate::runtime::Scheduler;
use crate::state::{self, RunState};

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
) -> Result<i32> {
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

    let (mut ir, plan_job_id) = if do_skip {
        if auto_skip && !skip_plan {
            println!(
                "skip-plan: structured plan (adapter={adapter_name}) → direct exec"
            );
        } else if skip_plan {
            println!("skip-plan: forced (--skip-plan)");
        }
        let ir = load_plan(&project, &plan, adapter.as_deref(), &config)?;
        (ir, None)
    } else {
        println!(
            "planning… (adapter={adapter_name} is not structured; Mode B plan job)"
        );
        let (ir, job_id) = plan_then_load_ir(
            &config,
            &project,
            &plan,
            &plan_mode,
            planner_provider,
            mode.clone(),
            max_parallel,
        )?;
        (ir, Some(job_id))
    };

    // P1-7: --provider soft-fill vs --force-provider full wipe (see apply_provider_override).
    if let Some(msg) = apply_provider_override(&mut ir, provider, force_provider) {
        println!("{msg}");
    }
    if let Some(m) = mode {
        for t in &mut ir.tasks {
            t.mode = m.clone();
        }
        ir.default_mode = m;
    }
    if let Some(mp) = max_parallel {
        ir.max_parallel = mp;
    }

    print!("{}", format_graph(&ir));
    if !yes && !dry_run {
        if !interactive::confirm("proceed?", false)? {
            println!("aborted");
            return Ok(1);
        }
    }

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let run_state =
        RunState::new(run_id.clone(), project.canonicalize()?, &ir, run_dir.clone());
    run_state.save()?;

    if let Some(ref job_id) = plan_job_id {
        let _ = crate::plan::planner::mark_confirmed(&config, job_id, &run_id, &ir);
    }

    println!("run_id: {run_id}");
    println!("run_dir: {}", run_state.run_dir.display());

    let registry = ProviderRegistry::from_config(&config)?;
    preflight_providers(&registry, &ir).await?;

    let mirror = if mirror_state || config.default.mirror_state {
        let m = project.join(".cco").join("runs");
        std::fs::create_dir_all(&m)?;
        Some(m)
    } else {
        None
    };

    let term_kind = resolve_term_kind(terminal_kind, &config);
    let auto_open = auto_open_terminal || config.terminal.auto_open_on_start;
    let tm = make_terminal_manager(&run_dir, &config);
    let poll = poll_interval(&config);
    let budget = max_budget.or(config.default.run_max_budget_usd);
    let provider_caps = provider_parallel_caps(&config);

    let sched = Scheduler {
        max_parallel: max_parallel.unwrap_or(ir.max_parallel),
        plan: ir,
        state: run_state,
        registry,
        poll_interval: poll,
        yes,
        only: only.map(|v| v.into_iter().collect()),
        from_task,
        dry_run,
        mirror_state: mirror,
        auto_open_terminal: auto_open,
        terminal_kind: term_kind,
        terminal_manager: Some(tm),
        run_max_budget_usd: budget,
        provider_max_parallel: provider_caps,
        retry_max: config.default.retry_max,
        stall_secs: config.default.stall_secs,
        failover_enabled: config.default.failover_enabled,
        fallback_extra_attempts: config.default.fallback_extra_attempts,
    };

    run_scheduler(sched, &config, &run_id, tui).await
}

/// P1-7 provider override semantics for `cco run`.
///
/// | flag | behavior |
/// |------|----------|
/// | none | no change |
/// | `--provider P` | set `default_provider = P`; rewrite only tasks whose provider is still
///   the old default (or the literal placeholder `"default"`). Tasks that
///   already declare a *different* engine keep it — mixed plans stay mixed. |
/// | `--force-provider P` | legacy full wipe: every `task.provider` + `default_provider = P`. |
///
/// When both flags are set, `--force-provider` wins (hard wipe).
/// Returns a short log line when an override was applied.
pub(crate) fn apply_provider_override(
    ir: &mut PlanIR,
    provider: Option<String>,
    force_provider: Option<String>,
) -> Option<String> {
    if let Some(p) = force_provider {
        for t in &mut ir.tasks {
            t.provider = p.clone();
        }
        ir.default_provider = p.clone();
        return Some(format!(
            "force-provider: all {} task(s) → {p}",
            ir.tasks.len()
        ));
    }
    if let Some(p) = provider {
        let old = ir.default_provider.clone();
        let mut filled = 0usize;
        let mut kept = 0usize;
        for t in &mut ir.tasks {
            // "still on default" = equals prior default_provider, or explicit placeholder.
            if t.provider == old || t.provider.eq_ignore_ascii_case("default") {
                t.provider = p.clone();
                filled += 1;
            } else {
                kept += 1;
            }
        }
        ir.default_provider = p.clone();
        return Some(format!(
            "provider: default → {p} (filled {filled} default task(s), kept {kept} explicit)"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{OnFailure, TaskIR};
    use std::path::PathBuf;

    fn mixed_plan() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "mixed".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("mixed.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            // Plan-level default; t1 inherits it, t2 declares codex, t3 uses placeholder.
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![
                task("t1", "claude"),
                task("t2", "codex"),
                task("t3", "default"),
            ],
        }
    }

    fn task(id: &str, provider: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: provider.into(),
            mode: "print".into(),
            prompt: "p".into(),
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
        tags: vec![],
        }
    }

    #[test]
    fn no_flag_leaves_plan_untouched() {
        let mut ir = mixed_plan();
        let msg = apply_provider_override(&mut ir, None, None);
        assert!(msg.is_none());
        assert_eq!(ir.default_provider, "claude");
        assert_eq!(ir.tasks[0].provider, "claude");
        assert_eq!(ir.tasks[1].provider, "codex");
        assert_eq!(ir.tasks[2].provider, "default");
    }

    #[test]
    fn soft_provider_fills_default_only_keeps_explicit() {
        // Mixed plan + --provider fake must NOT wipe codex on t2.
        let mut ir = mixed_plan();
        let msg = apply_provider_override(&mut ir, Some("fake".into()), None);
        assert!(msg.as_deref().unwrap().contains("filled 2"));
        assert!(msg.as_deref().unwrap().contains("kept 1"));
        assert_eq!(ir.default_provider, "fake");
        assert_eq!(ir.tasks[0].provider, "fake"); // was old default claude
        assert_eq!(ir.tasks[1].provider, "codex"); // explicit — preserved
        assert_eq!(ir.tasks[2].provider, "fake"); // placeholder "default"
    }

    #[test]
    fn force_provider_wipes_all_tasks() {
        let mut ir = mixed_plan();
        let msg = apply_provider_override(&mut ir, None, Some("fake".into()));
        assert!(msg.as_deref().unwrap().contains("force-provider"));
        assert_eq!(ir.default_provider, "fake");
        assert!(ir.tasks.iter().all(|t| t.provider == "fake"));
    }

    #[test]
    fn force_provider_wins_over_soft_provider() {
        let mut ir = mixed_plan();
        let msg = apply_provider_override(
            &mut ir,
            Some("claude".into()),
            Some("fake".into()),
        );
        assert!(msg.as_deref().unwrap().contains("force-provider"));
        assert!(ir.tasks.iter().all(|t| t.provider == "fake"));
        assert_eq!(ir.default_provider, "fake");
    }
}
