//! clap CLI surface.
//!
//! [INPUT]: argv · Config · plan/runtime/state/terminal
//! [OUTPUT]: Commands 枚举 · execute → exit code
//! [POS]: CLI 命令面；Mode B `plan` + `run`；D4 handlers 在 commands/
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

mod commands;
pub(crate) mod interactive;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::config::Config;

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
        /// Also print Mermaid flowchart (P2-7 thin slice)
        #[arg(long)]
        mermaid: bool,
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
        /// Claude reasoning effort for the planner CLI call
        #[arg(long)]
        effort: Option<String>,
    },
    /// Execute a plan (structured → direct exec; prose → plan job then confirm)
    Run {
        /// Project root (interactive if omitted)
        #[arg(long)]
        project: Option<PathBuf>,
        /// Plan path relative to project or absolute (interactive if omitted)
        #[arg(long)]
        plan: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
        /// Force skip Mode B planning (parse adapters only). Structured plans auto-skip.
        #[arg(long, default_value_t = false)]
        skip_plan: bool,
        /// Planner mode when planning: ai | parse | fake (default ai)
        #[arg(long, default_value = "ai")]
        plan_mode: String,
        #[arg(long)]
        mode: Option<String>,
        /// Soft override: set default_provider; only fill tasks still on the old default
        /// (or placeholder "default"). Explicit per-task engines in a mixed plan are kept.
        /// For a hard wipe of every task, use --force-provider instead.
        #[arg(long)]
        provider: Option<String>,
        /// Hard override: set every task.provider + default_provider (legacy full wipe).
        /// Prefer --provider for mixed multi-engine plans so declared engines survive.
        #[arg(long)]
        force_provider: Option<String>,
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
        /// Claude reasoning effort: low | medium | high | xhigh | max | ultracode
        /// (ultracode = xhigh + multi-agent thoroughness). Overrides config / CCO_EFFORT.
        #[arg(long)]
        effort: Option<String>,
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
        Commands::Doctor { project } => commands::doctor::run(&config, project).await,
        Commands::Init { force } => commands::init::run(force),
        Commands::Plans { project } => commands::plans::run(&config, project),
        Commands::Parse {
            project,
            plan,
            adapter,
            mermaid,
        } => commands::parse::run(&config, project, plan, adapter, mermaid),
        Commands::Plan {
            project,
            plan,
            plan_mode,
            provider,
            mode,
            json,
            effort,
        } => commands::plan_cmd::run(
            &config, project, plan, plan_mode, provider, mode, json, effort,
        ),
        Commands::Run {
            project,
            plan,
            yes,
            skip_plan,
            plan_mode,
            mode,
            provider,
            force_provider,
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
            effort,
        } => {
            commands::run::run(
                &config,
                project,
                plan,
                yes,
                skip_plan,
                plan_mode,
                mode,
                provider,
                force_provider,
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
                effort,
            )
            .await
        }
        Commands::Resume {
            run_id,
            yes,
            max_parallel,
            tui,
            max_budget,
        } => commands::resume::run(&config, run_id, yes, max_parallel, tui, max_budget).await,
        Commands::Status { run_id } => commands::status::run(&config, run_id),
        Commands::Stop { run_id, task } => commands::stop::run(&config, run_id, task),
        Commands::Report { run_id } => commands::report::run(&config, run_id),
        Commands::Logs {
            run_id,
            task,
            follow,
        } => commands::logs::run(&config, run_id, task, follow).await,
        Commands::Term { cmd } => commands::term::run(&config, cmd),
        Commands::Tui { run_id } => commands::tui_cmd::run(&config, run_id),
    }
}
