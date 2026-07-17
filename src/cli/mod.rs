//! clap CLI surface.

mod interactive;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::Config;
use crate::doctor;
use crate::graph::format_graph;
use crate::plan::{self, load_plan, PlanIR};
use crate::report;
use crate::runtime::provider::ProviderRegistry;
use crate::runtime::Scheduler;
use crate::runtime::provider::TaskStatus;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

#[derive(Debug, Parser)]
#[command(name = "cco", version, about = "CLI orchestrator for agent CLIs (Claude first)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TermKindArg {
    Embedded,
    External,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check providers, PATH, API key, state dir
    Doctor {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Write ~/.cco/config.toml template
    Init {
        #[arg(long)]
        force: bool,
    },
    /// List plan files under project
    Plans {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Parse plan and print task graph
    Parse {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        plan: Option<PathBuf>,
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Mode B: analyze plan into a task DAG (does not start workers)
    Plan {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        plan: Option<PathBuf>,
        /// parse | fake | ai (default ai)
        #[arg(long, default_value = "ai")]
        plan_mode: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        mode: Option<String>,
        /// After planning, print path to plan.proposed.json
        #[arg(long)]
        json: bool,
    },
    /// Execute a plan
    Run {
        /// Project root (interactive if omitted)
        #[arg(long)]
        project: Option<PathBuf>,
        /// Plan path relative to project or absolute (interactive if omitted)
        #[arg(long)]
        plan: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        mode: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        max_parallel: Option<usize>,
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        mirror_state: bool,
        #[arg(long)]
        from_task: Option<String>,
        #[arg(long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        tui: bool,
        /// Auto-open a terminal session when each task starts
        #[arg(long)]
        auto_open_terminal: bool,
        /// Kind for --auto-open-terminal (default from config)
        #[arg(long, value_enum)]
        terminal_kind: Option<TermKindArg>,
        /// Override run-level total budget USD
        #[arg(long)]
        max_budget: Option<f64>,
    },
    /// Resume a paused/failed/aborted run from unfinished tasks
    Resume {
        run_id: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        max_parallel: Option<usize>,
        #[arg(long)]
        tui: bool,
        #[arg(long)]
        max_budget: Option<f64>,
    },
    /// Show run status
    Status {
        run_id: Option<String>,
    },
    /// Stop a run or task (kills worker pid when known)
    Stop {
        run_id: Option<String>,
        #[arg(long)]
        task: Option<String>,
    },
    /// Print report.md
    Report {
        run_id: Option<String>,
    },
    /// Tail / show logs
    Logs {
        run_id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        follow: bool,
    },
    /// Multi-terminal sessions for a run
    Term {
        #[command(subcommand)]
        cmd: TermCommands,
    },
    /// Attach multi-page TUI to a run (or latest)
    Tui {
        run_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TermCommands {
    /// Open terminal for a task (logs or interactive shell)
    Open {
        run_id: Option<String>,
        #[arg(long)]
        task: String,
        #[arg(long, value_enum, default_value = "external")]
        kind: TermKindArg,
        /// Open interactive shell in work_dir instead of following logs
        #[arg(long)]
        shell: bool,
    },
    /// List terminal sessions
    List {
        run_id: Option<String>,
    },
    /// Close a session by id
    Close {
        run_id: Option<String>,
        #[arg(long)]
        session: String,
    },
}

pub async fn execute(cli: Cli) -> Result<i32> {
    let config = Config::load()?;

    match cli.command {
        Commands::Doctor { project } => {
            let report = doctor::run_doctor(&config, project.as_deref()).await?;
            doctor::print_report(&report);
            // show detected terminal launcher
            let tm = TerminalManager::for_run(
                PathBuf::from("/tmp").as_path(),
                &config.terminal.external_launcher,
                config.terminal.external_command.clone(),
            );
            println!(
                "  [info] external_launcher     {} (prefer={})",
                tm.detected_launcher().as_str(),
                config.terminal.external_launcher
            );
            Ok(if report.ok { 0 } else { 1 })
        }
        Commands::Init { force } => {
            let path = Config::config_path();
            if path.exists() && !force {
                println!("already exists: {} (use --force)", path.display());
                return Ok(0);
            }
            Config::write_template(&path)?;
            println!("wrote {}", path.display());
            Ok(0)
        }
        Commands::Plans { project } => {
            let project = interactive::resolve_project(project, true)?;
            interactive::ensure_project_dir(&project)?;
            let plans = plan::list_plans(&project)?;
            if plans.is_empty() {
                println!("(no plans found under docs/ or .cco/)");
            } else {
                for p in plans {
                    if let Ok(rel) = p.strip_prefix(&project) {
                        println!("{}", rel.display());
                    } else {
                        println!("{}", p.display());
                    }
                }
            }
            Ok(0)
        }
        Commands::Parse {
            project,
            plan,
            adapter,
        } => {
            let project = interactive::resolve_project(project, true)?;
            interactive::ensure_project_dir(&project)?;
            let plan = interactive::resolve_plan(&project, plan, true)?;
            let ir = load_plan(&project, &plan, adapter.as_deref(), &config)?;
            print!("{}", format_graph(&ir));
            Ok(0)
        }
        Commands::Plan {
            project,
            plan,
            plan_mode,
            provider,
            mode,
            json,
        } => {
            let project = interactive::resolve_project(project, true)?;
            interactive::ensure_project_dir(&project)?;
            let plan = interactive::resolve_plan(&project, plan, true)?;
            let view = crate::services::start_plan_job(
                &config,
                crate::services::StartPlanJobRequest {
                    project: project.clone(),
                    plan: plan.clone(),
                    plan_mode: Some(plan_mode.clone()),
                    provider,
                    mode,
                },
            )?;
            // Poll if async (ai mode may return planning)
            let mut view = view;
            if view.status == "planning" {
                println!("planning… (job {})", view.job_id);
                for _ in 0..600 {
                    std::thread::sleep(Duration::from_millis(500));
                    view = crate::services::get_plan_job(&config, &view.job_id)?;
                    if view.status != "planning" {
                        break;
                    }
                    if !view.planner_log_tail.is_empty() {
                        // show last line progress
                        if let Some(last) = view.planner_log_tail.lines().last() {
                            eprint!("\r{}", last.chars().take(100).collect::<String>());
                        }
                    }
                }
                eprintln!();
            }
            if view.status == "plan_failed" {
                eprintln!("plan failed: {}", view.error.as_deref().unwrap_or("?"));
                if !view.planner_log_tail.is_empty() {
                    eprintln!("--- planner log ---\n{}", view.planner_log_tail);
                }
                return Ok(1);
            }
            println!("job_id: {}", view.job_id);
            println!(
                "status: {}  name: {}  tasks: {}  max_parallel: {}  mode: {}",
                view.status,
                view.plan_name.as_deref().unwrap_or("—"),
                view.task_count.unwrap_or(0),
                view.max_parallel.unwrap_or(0),
                view.plan_mode
            );
            for (i, layer) in view.layers.iter().enumerate() {
                println!("wave {}: {}", i + 1, layer.join(", "));
            }
            for t in &view.tasks {
                let deps = if t.depends_on.is_empty() {
                    "—".into()
                } else {
                    t.depends_on.join(",")
                };
                println!("  - {}  \"{}\"  depends=[{deps}]", t.id, t.title);
            }
            let proposed = crate::plan::planner::job_dir(&config, &view.job_id)
                .join("plan.proposed.json");
            println!("proposed: {}", proposed.display());
            if json {
                if let Ok(body) = std::fs::read_to_string(&proposed) {
                    println!("{body}");
                }
            }
            println!("next: open desktop App to confirm, or use proposed JSON as plan source");
            Ok(0)
        }
        Commands::Run {
            project,
            plan,
            yes,
            mode,
            provider,
            max_parallel,
            adapter,
            mirror_state,
            from_task,
            only,
            dry_run,
            tui,
            auto_open_terminal,
            terminal_kind,
            max_budget,
        } => {
            let project = interactive::resolve_project(project, true)?;
            interactive::ensure_project_dir(&project)?;
            let plan = interactive::resolve_plan(&project, plan, true)?;
            let mut ir = load_plan(&project, &plan, adapter.as_deref(), &config)?;

            if let Some(p) = provider {
                for t in &mut ir.tasks {
                    t.provider = p.clone();
                }
                ir.default_provider = p;
            }
            if let Some(m) = mode {
                for t in &mut ir.tasks {
                    t.mode = m.clone();
                }
                ir.default_mode = m;
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
            };

            run_scheduler(sched, &config, &run_id, tui).await
        }
        Commands::Resume {
            run_id,
            yes,
            max_parallel,
            tui,
            max_budget,
        } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            let mut st = RunState::load(&dir)?;
            if matches!(st.status, RunStatus::Running) {
                bail!("run {} is still marked running; stop it first", st.run_id);
            }
            let plan_path = dir.join("plan.resolved.json");
            if !plan_path.exists() {
                bail!("missing plan.resolved.json in {}", dir.display());
            }
            let ir: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)
                .context("parse plan.resolved.json")?;
            let n = st.prepare_for_resume();
            // clear .done for non-success so provider can re-run
            for (id, ts) in &st.tasks {
                if matches!(ts.status, TaskStatus::Pending) {
                    let done = dir.join("tasks").join(id).join(".done");
                    let _ = std::fs::remove_file(done);
                }
            }
            st.save()?;
            println!(
                "resume {} · reset {n} unfinished task(s) · project={}",
                st.run_id,
                st.project_root.display()
            );
            print!("{}", format_graph(&ir));
            if !yes && !interactive::confirm("resume?", true)? {
                println!("aborted");
                return Ok(1);
            }

            let registry = ProviderRegistry::from_config(&config)?;
            preflight_providers(&registry, &ir).await?;
            let tm = make_terminal_manager(&dir, &config);
            let poll = poll_interval(&config);
            let budget = max_budget.or(config.default.run_max_budget_usd);
            let provider_caps = provider_parallel_caps(&config);
            let run_id = st.run_id.clone();

            let sched = Scheduler {
                max_parallel: max_parallel.unwrap_or(ir.max_parallel),
                plan: ir,
                state: st,
                registry,
                poll_interval: poll,
                yes,
                only: None,
                from_task: None,
                dry_run: false,
                mirror_state: None,
                auto_open_terminal: false,
                terminal_kind: SessionKind::Embedded,
                terminal_manager: Some(tm),
                run_max_budget_usd: budget,
                provider_max_parallel: provider_caps,
            };
            run_scheduler(sched, &config, &run_id, tui).await
        }
        Commands::Status { run_id } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            let st = RunState::load(&dir)?;
            println!("run_id: {}", st.run_id);
            println!("status: {:?}", st.status);
            println!("project: {}", st.project_root.display());
            println!("plan: {}", st.plan_path.display());
            println!("dir: {}", st.run_dir.display());
            println!("tasks:");
            let mut ids: Vec<_> = st.tasks.keys().cloned().collect();
            ids.sort();
            for id in ids {
                let t = &st.tasks[&id];
                let wd = t
                    .work_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "—".into());
                println!(
                    "  {id}: {:?} provider={} cost={:?} work_dir={wd} terms={}",
                    t.status,
                    t.provider,
                    t.cost_usd,
                    t.terminals.len()
                );
            }
            Ok(0)
        }
        Commands::Stop { run_id, task } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            let mut st = RunState::load(&dir)?;
            let targets: Vec<String> = if let Some(t) = task {
                vec![t]
            } else {
                st.tasks
                    .iter()
                    .filter(|(_, t)| {
                        matches!(
                            t.status,
                            TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
                        )
                    })
                    .map(|(k, _)| k.clone())
                    .collect()
            };

            for tid in &targets {
                // kill pid from meta / state
                if let Some(ts) = st.tasks.get(tid) {
                    if let Some(pid) = ts.pid {
                        kill_pid(pid);
                        println!("killed pid {pid} for task {tid}");
                    }
                }
                let meta = dir.join("tasks").join(tid).join("meta.json");
                if meta.exists() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(
                        &std::fs::read_to_string(&meta).unwrap_or_default(),
                    ) {
                        if let Some(pid) = v.get("pid").and_then(|p| p.as_u64()) {
                            kill_pid(pid as u32);
                        }
                    }
                }
                // mark done flag so pollers exit
                let done = dir.join("tasks").join(tid).join(".done");
                let _ = std::fs::write(&done, "130");
                if let Some(ts) = st.tasks.get_mut(tid) {
                    ts.status = TaskStatus::Stopped;
                    ts.finished_at = Some(chrono::Utc::now());
                }
                // close terminals for task
                let tm = TerminalManager::for_run(
                    &dir,
                    &config.terminal.external_launcher,
                    config.terminal.external_command.clone(),
                );
                let _ = tm.close_task(tid);
            }

            if targets.is_empty() {
                st.status = RunStatus::Aborted;
                st.finished_at = Some(chrono::Utc::now());
                st.event("run_end", serde_json::json!({"status": "aborted"}))?;
                println!("no running tasks; marked run aborted: {}", st.run_id);
            } else if targets.len() == st.tasks.len()
                || st.tasks.values().all(|t| t.status.is_terminal())
            {
                st.status = RunStatus::Aborted;
                st.finished_at = Some(chrono::Utc::now());
                st.event(
                    "run_end",
                    serde_json::json!({"status": "aborted", "stopped_tasks": targets}),
                )?;
                println!("stopped tasks {:?}; run aborted", targets);
            } else {
                st.event(
                    "tasks_stopped",
                    serde_json::json!({"tasks": targets}),
                )?;
                println!("stopped tasks {:?}", targets);
            }
            st.save()?;
            Ok(0)
        }
        Commands::Report { run_id } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            report::print_report_md(&dir)?;
            Ok(0)
        }
        Commands::Logs {
            run_id,
            task,
            follow,
        } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            if let Some(task) = task {
                let p = dir.join("tasks").join(&task).join("stdout.json");
                let err = dir.join("tasks").join(&task).join("stderr.log");
                if follow {
                    // simple poll loop
                    let mut last_len = 0usize;
                    loop {
                        if p.exists() {
                            let text = std::fs::read_to_string(&p).unwrap_or_default();
                            if text.len() > last_len {
                                print!("{}", &text[last_len..]);
                                last_len = text.len();
                            }
                        }
                        let done = dir.join("tasks").join(&task).join(".done");
                        if done.exists() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                    if err.exists() {
                        let e = std::fs::read_to_string(&err).unwrap_or_default();
                        if !e.is_empty() {
                            eprintln!("--- stderr ---\n{e}");
                        }
                    }
                } else if p.exists() {
                    print!("{}", std::fs::read_to_string(p)?);
                } else if err.exists() {
                    print!("{}", std::fs::read_to_string(err)?);
                } else {
                    bail!("no logs for task {task}");
                }
            } else {
                let events = dir.join("events.jsonl");
                if events.exists() {
                    print!("{}", std::fs::read_to_string(events)?);
                } else {
                    println!("(no events)");
                }
            }
            Ok(0)
        }
        Commands::Term { cmd } => match cmd {
            TermCommands::Open {
                run_id,
                task,
                kind,
                shell,
            } => {
                let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
                let st = RunState::load(&dir)?;
                if !st.tasks.contains_key(&task) {
                    bail!("unknown task {task} in run {}", st.run_id);
                }
                let (cwd, stdout, stderr) = task_paths(&st, &task)?;
                let tm = TerminalManager::for_run(
                    &dir,
                    &config.terminal.external_launcher,
                    config.terminal.external_command.clone(),
                )
                .with_limits(config.terminal.max_embedded, config.terminal.max_external);

                let kind = match kind {
                    TermKindArg::Embedded => SessionKind::Embedded,
                    TermKindArg::External => SessionKind::External,
                };

                let session = if shell {
                    if matches!(kind, SessionKind::Embedded) {
                        bail!("--shell requires --kind external (embedded PTY lands in M3 TUI)");
                    }
                    tm.open_shell(&task, &cwd)?
                } else {
                    tm.open_follow_logs(&task, &cwd, &stdout, &stderr, kind)?
                };

                // update run state terminals list
                let mut st = st;
                if let Some(ts) = st.tasks.get_mut(&task) {
                    ts.terminals.push(session.id.clone());
                }
                st.save()?;
                st.event(
                    "terminal_open",
                    serde_json::json!({
                        "task_id": task,
                        "kind": session.kind,
                        "session_id": session.id,
                        "launcher": session.launcher,
                    }),
                )?;

                println!("session: {}", session.id);
                println!("kind: {:?}", session.kind);
                if let Some(l) = &session.launcher {
                    println!("launcher: {l}");
                }
                println!("cwd: {}", session.cwd.display());
                println!("cmd: {}", session.command);
                Ok(0)
            }
            TermCommands::List { run_id } => {
                let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
                let tm = TerminalManager::for_run(
                    &dir,
                    &config.terminal.external_launcher,
                    config.terminal.external_command.clone(),
                );
                let sessions = tm.list()?;
                if sessions.is_empty() {
                    println!("(no terminal sessions)");
                } else {
                    for s in sessions {
                        let mark = if s.closed { "closed" } else { "open" };
                        println!(
                            "{}  [{mark}]  task={}  {:?}  launcher={}  cwd={}",
                            s.id,
                            s.task_id,
                            s.kind,
                            s.launcher.as_deref().unwrap_or("-"),
                            s.cwd.display()
                        );
                    }
                }
                Ok(0)
            }
            TermCommands::Close { run_id, session } => {
                let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
                let tm = TerminalManager::for_run(
                    &dir,
                    &config.terminal.external_launcher,
                    config.terminal.external_command.clone(),
                );
                let s = tm.close(&session)?;
                println!("closed session {} (task={})", s.id, s.task_id);
                Ok(0)
            }
        },
        Commands::Tui { run_id } => {
            let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
            let opts = crate::tui::options_from_config(dir, &config);
            crate::tui::run_tui(opts)?;
            Ok(0)
        }
    }
}

fn task_paths(st: &RunState, task: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let task_dir = st.task_dir(task);
    let mut cwd = st.project_root.clone();
    let wd = task_dir.join("work_dir.json");
    if wd.exists() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(wd)?) {
            if let Some(p) = v.get("work_dir").and_then(|x| x.as_str()) {
                cwd = PathBuf::from(p);
            }
        }
    } else if let Some(ts) = st.tasks.get(task) {
        if let Some(p) = &ts.work_dir {
            cwd = p.clone();
        }
    }
    let stdout = task_dir.join("stdout.json");
    let stderr = task_dir.join("stderr.log");
    // ensure log files exist so tail -f works
    if !stdout.exists() {
        let _ = std::fs::write(&stdout, "");
    }
    if !stderr.exists() {
        let _ = std::fs::write(&stderr, "");
    }
    Ok((cwd, stdout, stderr))
}

fn poll_interval(config: &Config) -> Duration {
    if std::env::var("CCO_FAST_POLL").is_ok() {
        Duration::from_millis(50)
    } else {
        Duration::from_millis((config.default.poll_interval_secs.max(1) * 1000).min(5_000))
    }
}

fn resolve_term_kind(arg: Option<TermKindArg>, config: &Config) -> SessionKind {
    match arg {
        Some(TermKindArg::Embedded) => SessionKind::Embedded,
        Some(TermKindArg::External) => SessionKind::External,
        None => {
            if config.terminal.default_kind == "external" {
                SessionKind::External
            } else {
                SessionKind::Embedded
            }
        }
    }
}

fn make_terminal_manager(run_dir: &std::path::Path, config: &Config) -> TerminalManager {
    TerminalManager::for_run(
        run_dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    )
    .with_limits(config.terminal.max_embedded, config.terminal.max_external)
}

fn provider_parallel_caps(config: &Config) -> HashMap<String, usize> {
    config
        .providers
        .iter()
        .filter_map(|(name, pc)| pc.max_parallel.map(|n| (name.clone(), n)))
        .collect()
}

async fn preflight_providers(registry: &ProviderRegistry, ir: &PlanIR) -> Result<()> {
    let used: HashSet<_> = ir.tasks.iter().map(|t| t.provider.clone()).collect();
    for name in &used {
        let p = registry.get(name)?;
        if let Err(e) = p.preflight().await {
            bail!("provider {name} preflight failed: {e:#}");
        }
    }
    Ok(())
}

async fn run_scheduler(
    sched: Scheduler,
    config: &Config,
    run_id: &str,
    tui: bool,
) -> Result<i32> {
    if tui {
        let run_dir_tui = config.runs_dir().join(run_id);
        let config_tui = config.clone();
        let join = tokio::spawn(async move { sched.run().await });
        let opts = crate::tui::options_from_config(run_dir_tui, &config_tui);
        let tui_result = tokio::task::spawn_blocking(move || crate::tui::run_tui(opts)).await;
        match tui_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("tui error: {e:#}"),
            Err(e) => eprintln!("tui join error: {e:#}"),
        }
        let status = join
            .await
            .map_err(|e| anyhow::anyhow!("scheduler join: {e}"))??;
        let run_dir = config.runs_dir().join(run_id);
        let st = RunState::load(&run_dir)?;
        report::write_reports(&st)?;
        println!("\nstatus: {:?}", status);
        println!("report: {}", run_dir.join("report.md").display());
        return Ok(match status {
            RunStatus::Completed => 0,
            RunStatus::Paused => 2,
            _ => 1,
        });
    }

    let status = sched.run().await?;
    let run_dir = config.runs_dir().join(run_id);
    let st = RunState::load(&run_dir)?;
    report::write_reports(&st)?;
    println!("\nstatus: {:?}", status);
    println!("report: {}", run_dir.join("report.md").display());
    Ok(match status {
        RunStatus::Completed => 0,
        RunStatus::Paused => 2,
        _ => 1,
    })
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15);
            let _ = kill(pid as i32, 9);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}
