//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::{bail, Result};

use crate::cli::commands::common::task_paths;
use crate::cli::{TermCommands, TermKindArg};
use crate::config::Config;
use crate::state::{self, RunState};
use crate::terminal::{SessionKind, TerminalManager};

pub fn run(config: &Config, cmd: TermCommands) -> Result<i32> {
    match cmd {
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
    }
}
