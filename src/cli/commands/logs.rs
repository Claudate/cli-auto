//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::time::Duration;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::state;

pub async fn run(config: &Config, run_id: Option<String>, task: Option<String>, follow: bool) -> Result<i32> {
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
                    // File may shrink/rewrite between polls; never slice past len
                    // or mid-char (UTF-8).
                    if text.len() < last_len {
                        last_len = 0;
                    }
                    let from = crate::runtime::log_events::floor_char_boundary(
                        &text, last_len,
                    );
                    if text.len() > from {
                        print!("{}", &text[from..]);
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
