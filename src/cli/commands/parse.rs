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
use crate::graph::{format_graph, format_mermaid};
use crate::plan::load_plan;

pub fn run(
    config: &Config,
    project: Option<PathBuf>,
    plan: Option<PathBuf>,
    adapter: Option<String>,
    mermaid: bool,
) -> Result<i32> {
    let project = interactive::resolve_project(project, true)?;
    interactive::ensure_project_dir(&project)?;
    let plan = interactive::resolve_plan(&project, plan, true)?;
    let ir = load_plan(&project, &plan, adapter.as_deref(), config)?;
    print!("{}", format_graph(&ir));
    if mermaid {
        print!("\n{}", format_mermaid(&ir));
    }
    Ok(0)
}
