//! Auto-retry on fail + stall patrol → pause when exhausted.
//! H4: same-provider retry exhaust → optional provider failover (claude↔codex).
use std::sync::Arc;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::fake::FakeProvider;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};

fn sched(
    ir: cco::plan::PlanIR,
    run_state: RunState,
    registry: ProviderRegistry,
    retry_max: u32,
    stall_secs: u64,
    failover_enabled: bool,
    fallback_extra_attempts: u32,
) -> Scheduler {
    Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(30),
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
        retry_max,
        stall_secs,
        failover_enabled,
        fallback_extra_attempts,
        failover_order: vec![],
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
    }
}

fn read_events(run_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let path = run_dir.join("events.jsonl");
    let body = std::fs::read_to_string(&path).unwrap_or_default();
    body.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[tokio::test]
async fn fail_once_auto_retries_and_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/fail-once.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: fail-once
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: flaky
    prompt: |
      CCO_FAKE_FAIL_ONCE
      CCO_DONE ok
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

    // failover off: pure same-provider retry path (must not break existing semantics)
    let status = sched(
        ir, run_state, registry, /*retry_max*/ 2, 600, /*failover*/ false, 1,
    )
    .run()
    .await
    .unwrap();
    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert!(
        st.tasks["a"].attempt >= 2,
        "expected at least 2 attempts, got {}",
        st.tasks["a"].attempt
    );
    assert!(
        run_dir.join("tasks/a/attempt-1.stdout.json").exists()
            || run_dir.join("tasks/a/attempt-1.meta.json").exists(),
        "first attempt logs should be archived"
    );
}

#[tokio::test]
async fn stall_then_retry_exhausted_pauses() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hang.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: hang
defaults:
  provider: fake
  mode: print
tasks:
  - id: hang
    title: stuck
    prompt: |
      CCO_FAKE_HANG
      CCO_DONE ok
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

    // stall_secs=1 → detect quickly; retry_max=1 → 2 total attempts then pause
    // failover off (fake is not a production failover source)
    let status = sched(
        ir, run_state, registry, /*retry_max*/ 1, /*stall*/ 1, /*failover*/ false, 1,
    )
    .run()
    .await
    .unwrap();
    assert!(
        matches!(status, RunStatus::Paused | RunStatus::Failed),
        "expected pause after stall retries, got {status:?}"
    );
    let st = RunState::load(&run_dir).unwrap();
    assert!(
        matches!(
            st.tasks["hang"].status,
            TaskStatus::Timeout | TaskStatus::Failed
        ),
        "hang task should be terminal fail/timeout, got {:?}",
        st.tasks["hang"].status
    );
    assert!(
        st.tasks["hang"].attempt >= 2,
        "stall should have retried, attempt={}",
        st.tasks["hang"].attempt
    );
    assert_eq!(st.tasks["hang"].last_retry_reason.as_deref(), Some("stall"));
}

/// H4: hang under claude alias → same-house retries → switch to codex → succeed.
#[tokio::test]
async fn stall_exhaust_then_failover_switches_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/failover.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: failover
defaults:
  provider: claude
  mode: print
tasks:
  - id: flaky
    title: hang-then-switch
    prompt: |
      CCO_FAKE_HANG_UNTIL_FAILOVER
      CCO_DONE ok
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "claude".into();
    // Isolate registry: only fake aliases for claude/codex (no real CLIs).
    config.providers.get_mut("claude").unwrap().enabled = false;
    config.providers.get_mut("codex").unwrap().enabled = false;
    config.providers.get_mut("fake").unwrap().enabled = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[0].provider, "claude");
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let registry = ProviderRegistry::from_providers(vec![
        Arc::new(FakeProvider::with_name("inline".into(), "claude")),
        Arc::new(FakeProvider::with_name("inline".into(), "codex")),
    ])
    .unwrap();

    // retry_max=1 → 2 same-house attempts; then failover to codex (1 extra).
    let status = sched(
        ir, run_state, registry, /*retry_max*/ 1, /*stall*/ 1, /*failover*/ true,
        /*fallback_extra*/ 1,
    )
    .run()
    .await
    .unwrap();
    assert_eq!(status, RunStatus::Completed, "failover should succeed");
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["flaky"].status, TaskStatus::Done);
    assert_eq!(st.tasks["flaky"].provider, "codex");
    assert!(
        st.tasks["flaky"].failover_used,
        "run state should mark failover_used"
    );

    let events = read_events(&run_dir);
    let switched: Vec<_> = events
        .iter()
        .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("provider_switched"))
        .collect();
    assert_eq!(switched.len(), 1, "exactly one provider_switched event");
    assert_eq!(
        switched[0].get("from").and_then(|v| v.as_str()),
        Some("claude")
    );
    assert_eq!(
        switched[0].get("to").and_then(|v| v.as_str()),
        Some("codex")
    );

    let retries: Vec<_> = events
        .iter()
        .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("task_retry"))
        .collect();
    assert!(
        retries.iter().any(|e| {
            e.get("reason")
                .and_then(|r| r.as_str())
                .map(|r| r.starts_with("failover:"))
                .unwrap_or(false)
        }),
        "expected task_retry with failover:* reason, got {retries:?}"
    );
}

/// H4: failover disabled → no provider switch after same-house exhaust.
#[tokio::test]
async fn stall_exhaust_failover_disabled_no_switch() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/no-failover.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: no-failover
defaults:
  provider: claude
  mode: print
tasks:
  - id: hang
    title: stuck
    prompt: |
      CCO_FAKE_HANG
      CCO_DONE ok
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "claude".into();
    config.providers.get_mut("claude").unwrap().enabled = false;
    config.providers.get_mut("codex").unwrap().enabled = false;
    config.providers.get_mut("fake").unwrap().enabled = false;
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
    let registry = ProviderRegistry::from_providers(vec![
        Arc::new(FakeProvider::with_name("inline".into(), "claude")),
        Arc::new(FakeProvider::with_name("inline".into(), "codex")),
    ])
    .unwrap();

    let status = sched(
        ir, run_state, registry, /*retry_max*/ 1, /*stall*/ 1, /*failover*/ false, 1,
    )
    .run()
    .await
    .unwrap();
    assert!(
        matches!(status, RunStatus::Paused | RunStatus::Failed),
        "expected fail without switch, got {status:?}"
    );
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["hang"].provider, "claude");
    assert!(!st.tasks["hang"].failover_used);
    let events = read_events(&run_dir);
    assert!(
        events
            .iter()
            .all(|e| e.get("type").and_then(|t| t.as_str()) != Some("provider_switched")),
        "no provider_switched when failover disabled"
    );
}

/// H4: user-initiated stop never retries and never failovers.
#[tokio::test]
async fn manual_stop_no_retry_no_failover() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/stop.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: stop
defaults:
  provider: claude
  mode: print
tasks:
  - id: a
    title: stop-me
    prompt: |
      CCO_FAKE_STOP
      CCO_DONE ok
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "claude".into();
    config.providers.get_mut("claude").unwrap().enabled = false;
    config.providers.get_mut("codex").unwrap().enabled = false;
    config.providers.get_mut("fake").unwrap().enabled = false;
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
    let registry = ProviderRegistry::from_providers(vec![
        Arc::new(FakeProvider::with_name("inline".into(), "claude")),
        Arc::new(FakeProvider::with_name("inline".into(), "codex")),
    ])
    .unwrap();

    // Even with failover on + retry budget, stop must be terminal on first collect.
    let status = sched(
        ir, run_state, registry, /*retry_max*/ 2, 600, /*failover*/ true, 1,
    )
    .run()
    .await
    .unwrap();
    assert!(
        matches!(
            status,
            RunStatus::Aborted | RunStatus::Paused | RunStatus::Failed
        ),
        "stop should end the run (aborted/paused/failed), got {status:?}"
    );
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(
        st.tasks["a"].status,
        TaskStatus::Stopped,
        "manual stop → Stopped, got {:?}",
        st.tasks["a"].status
    );
    assert_eq!(st.tasks["a"].provider, "claude");
    assert!(!st.tasks["a"].failover_used);
    // First attempt only — no retry, no switch.
    assert_eq!(
        st.tasks["a"].attempt, 1,
        "stop must not retry; attempt={}",
        st.tasks["a"].attempt
    );
    let events = read_events(&run_dir);
    assert!(
        events
            .iter()
            .all(|e| e.get("type").and_then(|t| t.as_str()) != Some("provider_switched")),
        "stop must not emit provider_switched"
    );
    assert!(
        events
            .iter()
            .all(|e| e.get("type").and_then(|t| t.as_str()) != Some("task_retry")),
        "stop must not emit task_retry"
    );
}
