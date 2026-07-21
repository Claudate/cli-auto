//! Run disk materialization (A5-1 · S-run extract).
//!
//! [INPUT]: Config · project · PlanIR / plan path + adapter
//! [OUTPUT]: (run_id, RunState, PlanIR) · ParseOnly also loads then materializes
//! [POS]: app::run sub-module; does **not** spawn scheduler
//! [PROTOCOL]: Mode B IR only via split::confirm_materialize; ParseOnly is documented
//!   non–Mode B **but still** drops `optional && !include` (A0-R4 · D-T3-1) — same
//!   `materialize_selected_tasks` as confirm. Callers **must** schedule the returned IR.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::{materialize_selected_tasks, load_plan, PlanIR};
use crate::state::{self, RunState};

/// Disk materialization of a new run (run_id + run.json + plan.resolved.json).
///
/// Always runs [`materialize_selected_tasks`] so unselected optionals never land
/// in `run.json` / `plan.resolved.json` / the returned IR (A0-R4 · D-T3-1).
/// Does **not** spawn the scheduler.
///
/// Mode B callers obtain `ir` via [`crate::app::split::confirm_materialize`]
/// (which may already have dropped optionals — re-apply is idempotent).
/// ParseOnly / `--skip-plan` use this after soft-fill, or [`materialize_parse_only`].
///
/// Returns `(run_id, state, ir)` — **use the returned `ir` for `prepare_scheduler`**.
pub fn materialize_run(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
) -> Result<(String, RunState, PlanIR)> {
    if !project.is_dir() {
        bail!("项目路径不是目录: {}", project.display());
    }
    // Drop optional && !include before any disk write or scheduler handoff.
    let ir = materialize_selected_tasks(ir.clone())?;
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let project = project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", project.display()))?;
    let run_state = RunState::new(run_id.clone(), project, &ir, run_dir.clone());
    run_state.save()?;
    let resolved = run_dir.join("plan.resolved.json");
    std::fs::write(&resolved, serde_json::to_string_pretty(&ir)?)?;
    Ok((run_id, run_state, ir))
}

/// ParseOnly / structured / `--skip-plan`: load plan from disk and materialize.
///
/// **Not** Mode B. Documented ParseOnly path — does not create a plan job.
/// Soft-fill / force-provider must be applied by the caller via
/// [`super::apply_provider_override`] before this call (or pass already-patched IR via
/// [`materialize_run`] after load). Unselected optionals are still dropped (A0-R4).
pub fn materialize_parse_only(
    config: &Config,
    project: PathBuf,
    plan: &Path,
    adapter: Option<&str>,
) -> Result<(String, RunState, PlanIR)> {
    let ir = load_plan(&project, plan, adapter, config)?;
    materialize_run(config, project, &ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR};
    use crate::plan::PlanIR;

    fn test_cfg(root: &std::path::Path) -> Config {
        let mut c = Config::default();
        c.state_root = root.to_path_buf();
        c
    }

    fn sample_ir(project: &std::path::Path) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "opt-parseonly".into(),
            adapter: "cco-plan/v1".into(),
            source_path: project.join("docs/plans/opt.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                TaskIR {
                    id: "must".into(),
                    title: "必做".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "must\nCCO_DONE ok".into(),
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: false,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
                TaskIR {
                    id: "maybe".into(),
                    title: "可选（未勾选）".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "maybe\nCCO_DONE ok".into(),
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: true,
                    include: false,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
                TaskIR {
                    id: "maybe_on".into(),
                    title: "可选（已勾选）".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "maybe_on\nCCO_DONE ok".into(),
                    acceptance: None,
                    timeout_secs: None,
                    worktree: Some(false),
                    provider_opts: serde_json::json!({}),
                    optional: true,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                    tags: vec![],
                },
            ],
        }
    }

    /// D-T3-1 / A0-R4: ParseOnly materialize drops unselected optionals from disk + return IR.
    #[test]
    fn materialize_run_drops_unselected_optional() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("docs/plans")).unwrap();
        let cfg = test_cfg(tmp.path());
        let ir = sample_ir(&project);

        let (run_id, st, out) = materialize_run(&cfg, project, &ir).unwrap();
        assert!(!run_id.is_empty());
        assert!(out.task("must").is_some());
        assert!(out.task("maybe_on").is_some());
        assert!(
            out.task("maybe").is_none(),
            "unselected optional must not appear in returned IR"
        );
        assert!(!st.tasks.contains_key("maybe"));
        assert!(st.tasks.contains_key("must"));
        assert!(st.tasks.contains_key("maybe_on"));

        let resolved: PlanIR = serde_json::from_str(
            &std::fs::read_to_string(st.run_dir.join("plan.resolved.json")).unwrap(),
        )
        .unwrap();
        assert!(resolved.task("maybe").is_none());
        assert_eq!(resolved.tasks.len(), 2);
    }
}
