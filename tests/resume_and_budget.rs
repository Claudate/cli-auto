use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};
use cco::SessionKind;

#[tokio::test]
async fn resume_skips_done_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/r.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: r
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: a
    prompt: "a\nCCO_DONE ok"
  - id: b
    depends_on: [a]
    prompt: "b\nCCO_DONE ok"
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
    let mut run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    // simulate a already done, b failed
    run_state.tasks.get_mut("a").unwrap().status = TaskStatus::Done;
    run_state.tasks.get_mut("a").unwrap().cost_usd = Some(0.01);
    run_state.tasks.get_mut("b").unwrap().status = TaskStatus::Failed;
    run_state.status = RunStatus::Paused;
    run_state.save().unwrap();
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        serde_json::to_string_pretty(&ir).unwrap(),
    )
    .unwrap();
    // mark a done on disk
    std::fs::create_dir_all(run_dir.join("tasks/a")).unwrap();
    std::fs::write(run_dir.join("tasks/a/.done"), "0").unwrap();

    let n = run_state.prepare_for_resume();
    assert_eq!(n, 1); // only b
    assert_eq!(run_state.tasks["a"].status, TaskStatus::Done);
    assert_eq!(run_state.tasks["b"].status, TaskStatus::Pending);
    run_state.save().unwrap();

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(15),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: None,
        provider_max_parallel: Default::default(),
        retry_max: 0,
        stall_secs: 600,
        failover_enabled: false,
        fallback_extra_attempts: 1,
        failover_order: vec![],
        event_emitter: None,
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
    .run()
    .await
    .unwrap();

    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["b"].status, TaskStatus::Done);
}

/// Card「再跑一次」semantics: only the named failed task re-runs; Done stays Done.
#[tokio::test]
async fn manual_task_retry_only_resets_target() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/rtry.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: rtry
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: a
    prompt: "a\nCCO_DONE ok"
  - id: b
    depends_on: [a]
    prompt: "b\nCCO_DONE ok"
  - id: c
    depends_on: [b]
    prompt: "c\nCCO_DONE ok"
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
    let mut run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    run_state.tasks.get_mut("a").unwrap().status = TaskStatus::Done;
    run_state.tasks.get_mut("b").unwrap().status = TaskStatus::Failed;
    run_state.tasks.get_mut("c").unwrap().status = TaskStatus::Pending;
    run_state.status = RunStatus::Failed;
    run_state.save().unwrap();
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        serde_json::to_string_pretty(&ir).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(run_dir.join("tasks/a")).unwrap();
    std::fs::write(run_dir.join("tasks/a/.done"), "0").unwrap();

    // Only b is re-queued; c stays Pending but a stays Done (not re-run).
    run_state.prepare_task_retry("b").unwrap();
    assert_eq!(run_state.tasks["a"].status, TaskStatus::Done);
    assert_eq!(run_state.tasks["b"].status, TaskStatus::Pending);
    assert_eq!(
        run_state.tasks["b"].last_retry_reason.as_deref(),
        Some("manual")
    );
    // c was already Pending — prepare_task_retry refuses live Pending
    // (we only reset Failed). After reset, leave c as-is if it was pending.
    run_state.save().unwrap();

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(15),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: None,
        provider_max_parallel: Default::default(),
        retry_max: 0,
        stall_secs: 600,
        failover_enabled: false,
        fallback_extra_attempts: 1,
        failover_order: vec![],
        event_emitter: None,
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
    .run()
    .await
    .unwrap();

    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert_eq!(st.tasks["b"].status, TaskStatus::Done);
    assert_eq!(st.tasks["c"].status, TaskStatus::Done);
}

#[tokio::test]
async fn run_budget_pauses_after_spend() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/b.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: budget
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: a
    prompt: "a\nCCO_DONE ok"
  - id: b
    depends_on: [a]
    prompt: "b\nCCO_DONE ok"
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
    // each fake task costs 0.01; cap at 0.005 so after first task we pause
    let status = Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(15),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: Some(0.005),
        provider_max_parallel: Default::default(),
        retry_max: 0,
        stall_secs: 600,
        failover_enabled: false,
        fallback_extra_attempts: 1,
        failover_order: vec![],
        event_emitter: None,
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
    .run()
    .await
    .unwrap();

    assert_eq!(status, RunStatus::Paused);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    // b should not have completed under budget pause
    assert_ne!(st.tasks["b"].status, TaskStatus::Done);
}
