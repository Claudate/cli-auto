//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::cli::commands::common::kill_pid;
use crate::config::Config;
use crate::runtime::provider::TaskStatus;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::TerminalManager;

pub fn run(config: &Config, run_id: Option<String>, task: Option<String>) -> Result<i32> {
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
