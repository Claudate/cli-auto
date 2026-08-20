//! A0 behavior goldens (P2-17 / architecture-redesign §7 A0 / §11 A0-1).
//!
//! Red lines locked for the architecture rewrite (must stay green while A1+ moves code):
//! 1. Mode B confirm is the business start path (`confirm_start` → run dir).
//! 2. `stop_run` freezes **Pending** (and Running/Starting/Queued) → run Aborted.
//! 3. Provider soft-fill (job default / tags) never overwrites an explicit task route.
//! 4. Unselected optional tasks are dropped on confirm — never silent auto-start.
//! 5. ParseOnly / structured `materialize_run` also drops `optional && !include` (D-T3-1).
//!
//! Inventory: `docs/contracts/behavior-golden.md`.

use std::path::PathBuf;
use std::time::Duration;

use cco::config::Config;
use cco::plan::planner::{
    get_plan_job, load_proposed, start_plan_job, update_proposed_task, StartPlanJobRequest,
};
use cco::plan::{load_plan, PlanIR};
use cco::runtime::provider::TaskStatus;
use cco::services::{confirm_start, stop_run};
use cco::state::{self, RunState, RunStatus, TaskState};

fn wait_planned(cfg: &Config, job_id: &str) -> cco::plan::planner::PlanJobView {
    let mut view = get_plan_job(cfg, job_id).unwrap();
    for _ in 0..80 {
        if view.status != "planning" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        view = get_plan_job(cfg, job_id).unwrap();
    }
    view
}

fn wait_run_terminal(cfg: &Config, run_id: &str) {
    for _ in 0..200 {
        let dir = cfg.runs_dir().join(run_id);
        if let Ok(st) = state::RunState::load(&dir) {
            if matches!(
                st.status,
                RunStatus::Completed | RunStatus::Failed | RunStatus::Aborted | RunStatus::Paused
            ) {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn test_config(tmp: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.state_root = tmp.join("state");
    config.default.default_provider = "fake".into();
    config.default.worktree = false;
    config.default.poll_interval_secs = 1;
    // Keep system post tasks off so optional counts stay predictable.
    config.default.post_inspect_enabled = false;
    config.default.post_git_push_enabled = false;
    config.default.post_open_pr_enabled = false;
    // A0 soft-fill expectations use job provider=fake; cost-auto would rewrite mid tier.
    config.default.cost_route_enabled = false;
    config.default.cost_escalate_enabled = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();
    config
}

/// A0-R1: structured plan → plan job → `confirm_start` creates run_dir with run.json.
#[tokio::test]
async fn a0_confirm_start_is_mode_b_run_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hello.cco.yaml");
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plans/hello.cco.yaml");
    std::fs::copy(&example, &plan_path).unwrap();

    let cfg = test_config(tmp.path());
    let view = start_plan_job(
        &cfg,
        StartPlanJobRequest {
            project: project.clone(),
            plan: PathBuf::from("docs/plans/hello.cco.yaml"),
            plan_mode: Some("parse".into()),
            provider: Some("fake".into()),
            mode: Some("print".into()),
            max_parallel: Some(1),
            preserve_from_job_id: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        },
    )
    .unwrap();
    let view = wait_planned(&cfg, &view.job_id);
    assert_eq!(view.status, "planned", "err={:?}", view.error);

    let run_id = confirm_start(cfg.clone(), &view.job_id, None).unwrap();
    let run_dir = cfg.runs_dir().join(&run_id);
    assert!(
        run_dir.join("run.json").exists(),
        "confirm must materialize run.json"
    );
    assert!(
        run_dir.join("plan.resolved.json").exists(),
        "confirm must freeze plan.resolved.json"
    );

    let job_dir = cfg.state_root.join("plan_jobs").join(&view.job_id);
    let job: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(job_dir.join("job.json")).unwrap()).unwrap();
    assert_eq!(job["status"], "confirmed");
    assert_eq!(job["run_id"], run_id);

    wait_run_terminal(&cfg, &run_id);
}

/// A0-R2: `stop_run` marks Pending (and Running) tasks Stopped; run → Aborted.
#[test]
fn a0_stop_run_freezes_pending_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = test_config(tmp.path());
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();

    let run_id = "20260720T000000Z-a0st".to_string();
    let run_dir = state::prepare_run_dir(&cfg.runs_dir(), &run_id).unwrap();

    // Minimal PlanIR for RunState::new shape.
    let ir: PlanIR = serde_json::from_value(serde_json::json!({
        "schema": "cco-plan/v1",
        "name": "a0-stop",
        "adapter": "test",
        "source_path": project.join("p.md"),
        "max_parallel": 2,
        "on_failure": "pause",
        "retry_max": 0,
        "default_provider": "fake",
        "default_mode": "print",
        "worktree": false,
        "require_inspect": false,
        "tasks": [
            {
                "id": "wave1",
                "title": "running task",
                "depends_on": [],
                "provider": "fake",
                "mode": "print",
                "prompt": "x\nCCO_DONE ok",
                "provider_opts": {},
                "optional": false,
                "include": true
            },
            {
                "id": "wave2",
                "title": "still pending",
                "depends_on": ["wave1"],
                "provider": "fake",
                "mode": "print",
                "prompt": "y\nCCO_DONE ok",
                "provider_opts": {},
                "optional": false,
                "include": true
            }
        ]
    }))
    .unwrap();

    let mut rs = RunState::new(
        run_id.clone(),
        project.canonicalize().unwrap_or(project),
        &ir,
        run_dir.clone(),
    );
    rs.status = RunStatus::Running;
    rs.tasks.get_mut("wave1").unwrap().status = TaskStatus::Running;
    rs.tasks.get_mut("wave1").unwrap().pid = Some(9_999_999); // non-existent; kill is best-effort
    rs.tasks.get_mut("wave2").unwrap().status = TaskStatus::Pending;
    // Seed task dirs the way a live run would.
    for id in ["wave1", "wave2"] {
        let td = run_dir.join("tasks").join(id);
        std::fs::create_dir_all(&td).unwrap();
    }
    rs.save().unwrap();

    stop_run(&cfg, &run_id).unwrap();

    let after = RunState::load(&run_dir).unwrap();
    assert_eq!(after.status, RunStatus::Aborted, "full-run stop must Abort");
    assert_eq!(
        after.tasks["wave1"].status,
        TaskStatus::Stopped,
        "Running must stop"
    );
    assert_eq!(
        after.tasks["wave2"].status,
        TaskStatus::Stopped,
        "Pending must stop — otherwise later waves keep spawning"
    );
    assert!(after.finished_at.is_some());
    // .done markers so external_stop / collect paths see terminal
    assert!(run_dir.join("tasks/wave1/.done").exists());
    assert!(run_dir.join("tasks/wave2/.done").exists());
}

/// A0-R3: job-level soft provider fill keeps explicit task routes (codex stays codex).
#[tokio::test]
async fn a0_soft_fill_preserves_explicit_provider_on_confirm() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/mixed.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: mixed-soft
defaults:
  provider: claude
  mode: print
  # Multi-provider legal under worktree isolation (serial edges still preferred).
  worktree: true
tasks:
  - id: t-default
    title: uses plan default
    prompt: |
      default path
      CCO_DONE ok
  - id: t-codex
    title: explicit codex
    provider: codex
    depends_on: [t-default]
    prompt: |
      codex path
      CCO_DONE ok
"#,
    )
    .unwrap();

    let cfg = test_config(tmp.path());
    // Sanity: load_plan soft tag/default path leaves explicit codex.
    let loaded = load_plan(&project, &plan_path, Some("cco-plan/v1"), &cfg).unwrap();
    assert_eq!(loaded.task("t-codex").unwrap().provider, "codex");

    // Job worker default is fake — soft-fill should rewrite plan-default tasks only.
    let view = start_plan_job(
        &cfg,
        StartPlanJobRequest {
            project: project.clone(),
            plan: PathBuf::from("docs/plans/mixed.cco.yaml"),
            plan_mode: Some("parse".into()),
            provider: Some("fake".into()),
            mode: Some("print".into()),
            max_parallel: Some(1),
            preserve_from_job_id: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        },
    )
    .unwrap();
    let view = wait_planned(&cfg, &view.job_id);
    assert_eq!(view.status, "planned", "err={:?}", view.error);

    let proposed = load_proposed(&cfg, &view.job_id).unwrap();
    // After plan job soft-fill at write time, explicit codex remains.
    assert_eq!(
        proposed.task("t-codex").unwrap().provider,
        "codex",
        "soft-fill must not overwrite explicit route on proposed"
    );

    let run_id = confirm_start(cfg.clone(), &view.job_id, None).unwrap();
    let resolved_path = cfg.runs_dir().join(&run_id).join("plan.resolved.json");
    // Wait briefly for start_run_from_plan to write resolved (it writes before spawn).
    for _ in 0..40 {
        if resolved_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let text = std::fs::read_to_string(&resolved_path).expect("plan.resolved.json after confirm");
    let resolved: PlanIR = serde_json::from_str(&text).unwrap();
    assert_eq!(
        resolved.task("t-codex").unwrap().provider,
        "codex",
        "confirm soft-fill must keep explicit codex"
    );
    assert_eq!(
        resolved.task("t-default").unwrap().provider,
        "fake",
        "plan-default task soft-fills to job provider"
    );

    wait_run_terminal(&cfg, &run_id);
}

/// A0-R4: optional with include=false is dropped on confirm — not auto-started.
#[tokio::test]
async fn a0_optional_unselected_not_in_run_after_confirm() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/opt.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: optional-gate
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: core
    title: required core
    prompt: |
      core work
      CCO_DONE ok
  - id: polish
    title: polish docs
    optional: true
    include: false
    depends_on: [core]
    prompt: |
      optional polish
      CCO_DONE ok
"#,
    )
    .unwrap();

    let cfg = test_config(tmp.path());
    let view = start_plan_job(
        &cfg,
        StartPlanJobRequest {
            project: project.clone(),
            plan: PathBuf::from("docs/plans/opt.cco.yaml"),
            plan_mode: Some("parse".into()),
            provider: Some("fake".into()),
            mode: Some("print".into()),
            max_parallel: Some(1),
            preserve_from_job_id: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        },
    )
    .unwrap();
    let view = wait_planned(&cfg, &view.job_id);
    assert_eq!(view.status, "planned", "err={:?}", view.error);

    let proposed = load_proposed(&cfg, &view.job_id).unwrap();
    assert!(
        proposed.task("polish").unwrap().optional,
        "optional flag must survive plan job"
    );
    assert!(
        !proposed.task("polish").unwrap().include,
        "unselected optional stays include=false until user checks"
    );

    // Ensure UI patch path can keep it off (no silent flip).
    update_proposed_task(
        &cfg,
        &view.job_id,
        "polish",
        None,
        None,
        Some(false),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let run_id = confirm_start(cfg.clone(), &view.job_id, None).unwrap();
    let resolved_path = cfg.runs_dir().join(&run_id).join("plan.resolved.json");
    for _ in 0..40 {
        if resolved_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let resolved: PlanIR =
        serde_json::from_str(&std::fs::read_to_string(&resolved_path).unwrap()).unwrap();
    assert!(
        resolved.task("polish").is_none(),
        "unselected optional must be dropped on confirm, not auto-started"
    );
    assert!(resolved.task("core").is_some());

    wait_run_terminal(&cfg, &run_id);
    let st = RunState::load(&cfg.runs_dir().join(&run_id)).unwrap();
    assert!(!st.tasks.contains_key("polish"));
    assert!(st.tasks.contains_key("core"));
}

/// A0-R4b: unit-level materialize already covered in plan mod; lock empty-selection error.
#[test]
fn a0_materialize_rejects_all_optional_unselected() {
    use cco::plan::{materialize_selected_tasks, OnFailure, TaskIR};

    let mut only_opt = TaskIR {
        id: "only".into(),
        title: "only optional".into(),
        depends_on: vec![],
        group: None,
        provider: "fake".into(),
        mode: "print".into(),
        prompt: "p\nCCO_DONE ok".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: None,
        provider_opts: serde_json::json!({}),
        optional: true,
        include: false,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
        wait_for: vec![],
    };
    only_opt.title = cco::plan::normalize_optional_title(&only_opt.title, true);
    let plan = PlanIR {
        schema: "cco-plan/v1".into(),
        name: "empty".into(),
        adapter: "test".into(),
        source_path: PathBuf::from("x"),
        max_parallel: 1,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: "fake".into(),
        default_mode: "print".into(),
        worktree: false,
        require_inspect: false,
        tasks: vec![only_opt],
    };
    let err = materialize_selected_tasks(plan).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("没有选中") || msg.contains("至少"),
        "must refuse empty selection: {msg}"
    );
}

/// A0-R4c / D-T3-1: ParseOnly `materialize_run` drops unselected optionals (not only Mode B confirm).
#[test]
fn a0_parse_only_materialize_drops_unselected_optional() {
    use cco::app::run as run_uc;
    use cco::plan::{OnFailure, TaskIR};

    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let cfg = test_config(tmp.path());

    let ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name: "parseonly-opt".into(),
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
                verify_cmd: None,
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
                wait_for: vec![],
            },
            TaskIR {
                id: "maybe".into(),
                title: "可选未勾选".into(),
                depends_on: vec![],
                group: None,
                provider: "fake".into(),
                mode: "print".into(),
                prompt: "maybe\nCCO_DONE ok".into(),
                verify_cmd: None,
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
                wait_for: vec![],
            },
        ],
    };

    let (run_id, st, out) = run_uc::materialize_run(&cfg, project, &ir).unwrap();
    assert!(!run_id.is_empty());
    assert!(out.task("must").is_some());
    assert!(
        out.task("maybe").is_none(),
        "ParseOnly must drop unselected optional from returned IR"
    );
    assert!(!st.tasks.contains_key("maybe"));
    assert!(st.tasks.contains_key("must"));

    let resolved: PlanIR = serde_json::from_str(
        &std::fs::read_to_string(st.run_dir.join("plan.resolved.json")).unwrap(),
    )
    .unwrap();
    assert!(
        resolved.task("maybe").is_none(),
        "plan.resolved.json must not keep unselected optional"
    );
}

/// Document companion: Pending TaskState constructor stays Pending (stop target).
#[test]
fn a0_task_state_pending_is_stop_target() {
    let ts = TaskState::pending("fake", "print");
    assert_eq!(ts.status, TaskStatus::Pending);
}
