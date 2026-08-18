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
#[command(
    name = "cco",
    version,
    about = "CLI orchestrator for agent CLIs (Claude first)"
)]
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
        /// B2: run headless — no TUI, no interactive confirm (CI-friendly).
        /// Equivalent to forcing `--yes`; suppresses `proceed?`.
        #[arg(long, default_value_t = false)]
        headless: bool,
        /// B2: output format for the completion result (only `json` supported).
        /// Implies a clean stdout; log_events stay on stderr.
        #[arg(long)]
        output: Option<String>,
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
    /// Re-run one failed/stopped/timeout task (not re-split). Optional --provider switches channel.
    Retry {
        run_id: String,
        #[arg(long)]
        task: String,
        /// Switch the task to this provider before retry (e.g. codex, claude, deepseek).
        #[arg(long)]
        provider: Option<String>,
    },
    /// Show run status
    Status { run_id: Option<String> },
    /// Stop a run or task (kills worker pid when known)
    Stop {
        run_id: Option<String>,
        #[arg(long)]
        task: Option<String>,
    },
    /// Print report.md
    Report { run_id: Option<String> },
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
    Tui { run_id: Option<String> },
    /// Git operations: status / remote / identity / commit / push / pull / fetch / diff / log / stash / branch / doctor
    Git {
        #[command(subcommand)]
        cmd: GitCommands,
    },
}

/// `cco git` subcommands.
#[derive(Debug, Subcommand)]
pub enum GitCommands {
    /// Show git status for a project (branch / changes / remotes / identity)
    Status {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Manage configured remotes (国内 Gitee / 国外 GitHub …)
    Remote {
        #[command(subcommand)]
        cmd: GitRemoteCommands,
    },
    /// Set or show repo-local git identity (user.name / user.email)
    Identity {
        #[command(subcommand)]
        cmd: GitIdentityCommands,
    },
    /// Commit changes (auto-filters secrets/.env; optional push)
    Commit {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Commit message (required unless --dry-run)
        #[arg(short, long)]
        message: Option<String>,
        /// List files that would be committed without committing
        #[arg(long)]
        dry_run: bool,
        /// Push after successful commit
        #[arg(long)]
        push: bool,
        /// Add all changes (default); use --paths to add specific files
        #[arg(long, default_value_t = true)]
        all: bool,
        /// Specific paths to add (overrides --all)
        #[arg(long, value_delimiter = ',')]
        paths: Vec<String>,
        /// Allow force-push (only effective when config.git.auto_commit.allow_force is true)
        #[arg(long)]
        force: bool,
    },
    /// Push current branch to a remote
    Push {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Remote name (default: picked from config.git.default_region)
        #[arg(long)]
        remote: Option<String>,
        /// Branch (default: current)
        #[arg(long)]
        branch: Option<String>,
        /// Force-push (only if config.git.auto_commit.allow_force)
        #[arg(long)]
        force: bool,
    },
    /// Check git environment (binary / repo / remotes / identity)
    Doctor {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Pull from a remote (fetch + rebase/merge)
    Pull {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        remote: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        /// merge | rebase | fail (default: rebase)
        #[arg(long, default_value = "rebase")]
        strategy: String,
    },
    /// Fetch from a remote
    Fetch {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        remote: Option<String>,
        #[arg(long)]
        prune: bool,
    },
    /// Show commit log
    Log {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Number of entries (default 20, max 200)
        #[arg(long, default_value = "20")]
        n: usize,
        #[arg(long)]
        oneline: bool,
    },
    /// Show diff of working tree
    Diff {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        staged: bool,
        #[arg(long)]
        stat: bool,
        /// Changed files only
        #[arg(long)]
        name_only: bool,
    },
    /// Stash changes
    Stash {
        #[command(subcommand)]
        cmd: GitStashCommands,
    },
    /// Manage branches (list / create / switch / delete)
    Branch {
        #[command(subcommand)]
        cmd: GitBranchCommands,
    },
    /// Manage tags (list / create / delete / show)
    Tag {
        #[command(subcommand)]
        cmd: GitTagCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitBranchCommands {
    /// List local branches
    List {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Create a new branch
    Create {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
        #[arg(long)]
        base: Option<String>,
    },
    /// Switch to a branch
    Switch {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
    },
    /// Delete a local branch
    Delete {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitStashCommands {
    /// List stash entries
    List {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Push current changes to stash
    Push {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Stash message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Pop the latest stash entry (apply + drop)
    Pop {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Stash index (default: 0)
        #[arg(long)]
        index: Option<usize>,
    },
    /// Apply a stash entry without removing it
    Apply {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        index: Option<usize>,
    },
    /// Drop a stash entry
    Drop {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        index: Option<usize>,
    },
    /// Show the diff of a stash entry
    Show {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        index: Option<usize>,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitRemoteCommands {
    /// List configured remotes
    List,
    /// Add or update a remote in config (国内|国外)
    Add {
        name: String,
        url: String,
        /// domestic | overseas (aliases: cn/国内/github/海外…)
        #[arg(long, default_value = "overseas")]
        region: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// Remove a remote from config (does not touch git itself)
    Remove { name: String },
    /// Apply configured remotes to the actual git repo (git remote add/set-url)
    Apply {
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitIdentityCommands {
    /// Set repo-local user.name / user.email (never touches --global)
    Set {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        email: Option<String>,
    },
    /// Show current identity
    Show {
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitTagCommands {
    /// List tags
    List {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Create a lightweight tag
    Create {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
        /// Commit/ref to tag (default: HEAD)
        #[arg(long)]
        commit: Option<String>,
    },
    /// Create an annotated tag with a message
    Annotate {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
        #[arg(short, long)]
        message: String,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Delete a tag
    Delete {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
    },
    /// Show details of a tag
    Show {
        #[arg(long)]
        project: Option<PathBuf>,
        name: String,
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
    List { run_id: Option<String> },
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
            headless,
            output,
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
                headless,
                output,
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
        Commands::Retry { run_id, task, provider } => {
            commands::retry::run(&config, run_id, task, provider)
        }
        Commands::Stop { run_id, task } => commands::stop::run(&config, run_id, task),
        Commands::Report { run_id } => commands::report::run(&config, run_id),
        Commands::Logs {
            run_id,
            task,
            follow,
        } => commands::logs::run(&config, run_id, task, follow).await,
        Commands::Term { cmd } => commands::term::run(&config, cmd),
        Commands::Tui { run_id } => commands::tui_cmd::run(&config, run_id),
        Commands::Git { cmd } => commands::git::run(&config, cmd),
    }
}
