//! P1-4: run_dir handoff.md/json — host ledger + outputs gate (fake provider).

use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::handoff::Handoff;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};

fn make_scheduler(
    ir: cco::plan::PlanIR,
    run_state: RunState,
    registry: ProviderRegistry,
) -> Scheduler {
    Scheduler {
        max_parallel: 2,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(20),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: cco::SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: None,
        provider_max_parallel: Default::default(),
        retry_max: 0,
        stall_secs: 600,
        failover_enabled: false,
        fallback_extra_attempts: 1,
    }
}

/// Two-task happy path: handoff shell → Board done + Fragments merged.
#[tokio::test]
async fn handoff_updates_across_two_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/handoff-ok.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: handoff-ok
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
  - id: b
    title: b
    depends_on: [a]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    assert!(run_dir.join("handoff.md").exists());
    assert!(run_dir.join("handoff.json").exists());

    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.status, "completed");
    assert_eq!(h.board.len(), 2);
    assert!(h.board.iter().all(|r| r.status == "done"));
    assert!(h.fragments.contains_key("a"));
    assert!(h.fragments.contains_key("b"));
    assert_eq!(h.fragments["a"].status, "done");
    assert_eq!(h.fragments["b"].provider, "fake");

    let md = std::fs::read_to_string(run_dir.join("handoff.md")).unwrap();
    assert!(md.contains("## Board"));
    assert!(md.contains("## Timeline"));
    assert!(md.contains("## Fragments"));
    assert!(md.contains("## Open risks"));
    assert!(md.contains("## Instructions for next worker"));
    assert!(md.contains("task_start") || md.contains("task_end") || md.contains("run_"));
}

/// Declared outputs missing → task Failed (not Done); handoff Board reflects fail.
#[tokio::test]
async fn missing_outputs_fails_task_and_updates_handoff() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/handoff-miss.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: handoff-miss
on_failure: continue
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a without writing outputs\nCCO_DONE ok"
    outputs:
      - .cco-out/a/SUMMARY.md
  - id: b
    title: b
    depends_on: [a]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[0].outputs, vec![".cco-out/a/SUMMARY.md".to_string()]);

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    // a fails (missing outputs); b depends on a → skipped under continue
    assert!(
        matches!(status, RunStatus::Failed | RunStatus::Paused | RunStatus::Completed),
        "status={status:?}"
    );

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Failed);
    let err = st.tasks["a"].error.as_deref().unwrap_or("");
    assert!(
        err.contains("missing outputs"),
        "expected missing outputs error, got: {err}"
    );

    let h = Handoff::load(&run_dir).unwrap();
    let row_a = h.board.iter().find(|r| r.id == "a").unwrap();
    assert_eq!(row_a.status, "failed");
    assert!(h.fragments.contains_key("a"));
    assert_eq!(h.fragments["a"].status, "failed");
    assert!(
        h.open_risks.iter().any(|r| r.contains("a") || r.contains("missing")),
        "open_risks={:?}",
        h.open_risks
    );
}

/// When declared outputs already exist under project_root, task stays Done and fragment lists artifacts.
#[tokio::test]
async fn present_outputs_marks_done_and_lists_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/a")).unwrap();
    std::fs::write(project.join(".cco-out/a/SUMMARY.md"), "a done\n").unwrap();

    let plan_path = project.join("docs/plans/handoff-out.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: handoff-out
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
    outputs:
      - .cco-out/a/SUMMARY.md
  - id: b
    title: b
    depends_on: [a]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert_eq!(st.tasks["b"].status, TaskStatus::Done);

    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.board.iter().find(|r| r.id == "a").unwrap().status, "done");
    assert!(h.fragments["a"].artifacts.iter().any(|a| a.contains("SUMMARY")));
    assert!(
        h.fragments["a"].summary.contains("a done") || !h.fragments["a"].summary.is_empty(),
        "summary={:?}",
        h.fragments["a"].summary
    );
}

/// P2-3: pre-seeded VERDICT=FAIL + ISSUES → task Failed, run Paused, handoff Open risks has ISSUES.
#[tokio::test]
async fn inspect_verdict_fail_pauses_and_records_issues() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    // Fake provider does not write files; pre-seed inspect products under project_root.
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/VERDICT.md"),
        "FAIL\nscope leak: feat-a wrote into demo_b\n",
    )
    .unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/ISSUES.md"),
        "- file: examples/demo_b/extra.rs\n- symptom: written by feat-a (out of scope)\n- suggestion: remove file; re-run feat-a within scope\n",
    )
    .unwrap();

    let plan_path = project.join("docs/plans/inspect-fail.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: inspect-fail
on_failure: pause
retry_max: 0
defaults:
  provider: fake
  mode: print
tasks:
  - id: implement
    title: stub implement
    role: implement
    scope:
      paths: [src/**]
    prompt: "stub\nCCO_DONE ok"
  - id: inspect
    title: inspect gate
    role: inspect
    depends_on: [implement]
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect only\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[1].role, Some(cco::plan::TaskRole::Inspect));

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Paused, "VERDICT=FAIL must pause under on_failure=pause");

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["implement"].status, TaskStatus::Done);
    assert_eq!(st.tasks["inspect"].status, TaskStatus::Failed);
    let err = st.tasks["inspect"].error.as_deref().unwrap_or("");
    assert!(
        err.contains("VERDICT=FAIL") || err.contains("inspect VERDICT"),
        "expected VERDICT fail error, got: {err}"
    );

    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.status, "paused");
    let row = h.board.iter().find(|r| r.id == "inspect").unwrap();
    assert_eq!(row.status, "failed");
    assert!(
        h.open_risks
            .iter()
            .any(|r| r.contains("ISSUES[inspect]") || r.contains("scope")),
        "open_risks must carry ISSUES clue, got {:?}",
        h.open_risks
    );
    assert!(
        h.open_risks.iter().any(|r| r.contains("REWORK_HOOK"))
            || h.instructions_for_next.contains("REWORK_HOOK")
            || h.instructions_for_next.contains("ISSUES"),
        "rework hook or ISSUES must surface in handoff; risks={:?} next={}",
        h.open_risks,
        h.instructions_for_next
    );

    let md = std::fs::read_to_string(run_dir.join("handoff.md")).unwrap();
    assert!(md.contains("## Open risks"));
    assert!(
        md.contains("ISSUES") || md.contains("REWORK_HOOK"),
        "handoff.md should mention ISSUES/REWORK: {md}"
    );
}

/// P2-3: VERDICT=PASS keeps Done and does not pollute Open risks with REWORK_HOOK.
#[tokio::test]
async fn inspect_verdict_pass_completes() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(project.join(".cco-out/inspect/VERDICT.md"), "PASS\nall good\n").unwrap();
    std::fs::write(project.join(".cco-out/inspect/ISSUES.md"), "无\n").unwrap();

    let plan_path = project.join("docs/plans/inspect-pass.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: inspect-pass
on_failure: pause
retry_max: 0
defaults:
  provider: fake
  mode: print
tasks:
  - id: inspect
    title: inspect gate
    role: inspect
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect only\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["inspect"].status, TaskStatus::Done);

    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.status, "completed");
    assert!(
        !h.open_risks.iter().any(|r| r.contains("REWORK_HOOK")),
        "PASS should not register rework hook: {:?}",
        h.open_risks
    );
}

/// P-loop: VERDICT=PASS but severity=blocking ISSUES → task Failed (no silent residual PASS).
#[tokio::test]
async fn inspect_pass_with_blocking_issues_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/VERDICT.md"),
        "Result: PASS\nlooks fine\n",
    )
    .unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/ISSUES.md"),
        "- id: I-1\n  severity=map\n  plan_ref: §8\n  path: CLAUDE.md\n  symptom: L1 stale\n  fix_wp: update pointer\n",
    )
    .unwrap();

    let plan_path = project.join("docs/plans/inspect-pass-block.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: inspect-pass-block
on_failure: pause
retry_max: 0
defaults:
  provider: fake
  mode: print
tasks:
  - id: inspect
    title: inspect gate
    role: inspect
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect only\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Paused);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["inspect"].status, TaskStatus::Failed);
    let err = st.tasks["inspect"].error.as_deref().unwrap_or("");
    assert!(
        err.contains("blocking") || err.contains("PASS but"),
        "expected blocking residual gate, got: {err}"
    );
}

/// P-loop: residual-only ISSUES under PASS → Completed (non-blocking appendix).
#[tokio::test]
async fn inspect_pass_with_residual_only_completes() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(project.join(".cco-out/inspect/VERDICT.md"), "PASS\n").unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/ISSUES.md"),
        "- id: I-opt\n  severity=residual\n  plan_ref: F2\n  symptom: optional polish not done\n  fix_wp: later\n",
    )
    .unwrap();

    let plan_path = project.join("docs/plans/inspect-residual.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: inspect-residual
on_failure: pause
defaults:
  provider: fake
  mode: print
tasks:
  - id: inspect
    title: inspect gate
    role: inspect
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect only\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["inspect"].status, TaskStatus::Done);
}

/// P-loop: build_rework_plan from FAIL ISSUES → valid DAG with require_inspect.
#[test]
fn rework_plan_from_inspect_issues_validates() {
    use cco::runtime::handoff::{build_rework_plan, parse_issues_text};

    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/base.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: base-for-rework
require_inspect: true
defaults:
  provider: fake
  mode: print
tasks:
  - id: implement
    title: impl
    role: implement
    scope:
      paths: [src/**]
    prompt: "x\nCCO_DONE ok"
  - id: inspect
    title: inspect
    role: inspect
    depends_on: [implement]
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "y\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    let base = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let issues = parse_issues_text(
        "- id: I-1\n  severity=blocking\n  plan_ref: S1\n  path: src/lib.rs\n  symptom: missing\n  fix_wp: add fn\n",
    );
    assert!(!issues.is_empty());
    let rework = build_rework_plan(&base, &issues, 1, "src-run").unwrap();
    assert!(rework.require_inspect);
    assert_eq!(rework.tasks.len(), 2);
    assert_eq!(rework.tasks[0].role, Some(cco::plan::TaskRole::Implement));
    assert_eq!(rework.tasks[1].role, Some(cco::plan::TaskRole::Inspect));
    assert!(rework.tasks[0].prompt.contains("I-1") || rework.tasks[0].prompt.contains("S1"));
    rework.validate().unwrap();
}
