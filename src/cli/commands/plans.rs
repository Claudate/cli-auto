//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;

use anyhow::Result;

use crate::cli::interactive;
use crate::config::Config;
use crate::plan;

pub fn run(_config: &Config, project: Option<PathBuf>) -> Result<i32> {
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
