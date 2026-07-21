//! Run disk materialization (A5-1 · S-run extract).
//!
//! [INPUT]: Config · project · PlanIR / plan path + adapter
//! [OUTPUT]: (run_id, RunState) · ParseOnly also returns PlanIR
//! [POS]: app::run sub-module; does **not** spawn scheduler
//! [PROTOCOL]: Mode B IR only via split::confirm_materialize; ParseOnly is documented non–Mode B

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::{load_plan, PlanIR};
use crate::state::{self, RunState};

/// Disk materialization of a new run (run_id + run.json + plan.resolved.json).
///
/// Does **not** spawn the scheduler. Mode B callers must obtain `ir` only via
/// [`crate::app::split::confirm_materialize`] (optional drop + soft defaults).
/// ParseOnly / `--skip-plan` use [`materialize_parse_only`].
pub fn materialize_run(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
) -> Result<(String, RunState)> {
    if !project.is_dir() {
        bail!("项目路径不是目录: {}", project.display());
    }
    ir.validate()?;
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let project = project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", project.display()))?;
    let run_state = RunState::new(run_id.clone(), project, ir, run_dir.clone());
    run_state.save()?;
    let resolved = run_dir.join("plan.resolved.json");
    std::fs::write(&resolved, serde_json::to_string_pretty(ir)?)?;
    Ok((run_id, run_state))
}

/// ParseOnly / structured / `--skip-plan`: load plan from disk and materialize.
///
/// **Not** Mode B. Documented ParseOnly path — does not create a plan job.
/// Soft-fill / force-provider must be applied by the caller via
/// [`super::apply_provider_override`] before this call (or pass already-patched IR via
/// [`materialize_run`] after load).
pub fn materialize_parse_only(
    config: &Config,
    project: PathBuf,
    plan: &Path,
    adapter: Option<&str>,
) -> Result<(String, RunState, PlanIR)> {
    let ir = load_plan(&project, plan, adapter, config)?;
    let (run_id, st) = materialize_run(config, project, &ir)?;
    Ok((run_id, st, ir))
}
