//! Ensure E1/E3: closeout inject + docs-only FAIL → auto rework (fake provider).
//!
//! Locks the wros-shaped loop: implement (role=None ok) + inspect FAIL on
//! ledger/map ISSUES → host starts a new rework run without user click.

use std::time::Duration;

use cco::app::run::{materialize_run_with_route, maybe_auto_rework};
use cco::config::Config;
use cco::plan::{load_plan, TaskRole, SYS_CLOSEOUT_ID};
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
        failover_order: vec![],
        event_emitter: None,
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
}

const DOCS_ISSUES: &str = r#"
### B6
- severity=blocking
- plan_ref: §9
- path: docs/gap-audit.md
- symptom: 台账 §6/§9/README 仍「未开工」
- fix_wp: 回写台账勾选与进度句

### M1
- severity=map
- plan_ref: acceptance
- path: docs/acceptance/README.md
- symptom: acceptance README 断链
- fix_wp: 修索引指针
"#;

const BUSINESS_ISSUES: &str = r#"
### B1
- severity=blocking
- plan_ref: engine
- path: src/runtime/scheduler/mod.rs
- symptom: 引擎未实现 failover 分支
- fix_wp: 实现 scheduler failover
"#;

/// E1: materialize injects `sys-closeout` for role=None business + inspect (wros shape).
#[test]
fn materialize_injects_closeout_for_role_none_gate() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/wros-shape.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: wros-shape
on_failure: pause
defaults:
  provider: fake
  mode: print
tasks:
  - id: t1
    title: 实现 A
    prompt: "do a\nCCO_DONE ok"
  - id: t2
    title: 实现 B
    depends_on: [t1]
    prompt: "do b\nCCO_DONE ok"
  - id: t7-p0-gates
    title: 门禁验收并回写台账
    role: inspect
    depends_on: [t2]
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect and rewrite ledger\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.default.cost_route_enabled = false;
    config.default.cost_escalate_enabled = false;
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.auto_closeout = true;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let (run_id, _state, resolved, _) =
        materialize_run_with_route(&config, project.clone(), &ir, None).unwrap();

    assert!(
        resolved.tasks.iter().any(|t| t.id == SYS_CLOSEOUT_ID),
        "sys-closeout must be injected; tasks={:?}",
        resolved.tasks.iter().map(|t| &t.id).collect::<Vec<_>>()
    );
    let co = resolved
        .tasks
        .iter()
        .find(|t| t.id == SYS_CLOSEOUT_ID)
        .unwrap();
    assert_eq!(co.role, Some(TaskRole::Closeout));
    assert!(co.depends_on.iter().any(|d| d == "t1") || co.depends_on.iter().any(|d| d == "t2"));

    let insp = resolved
        .tasks
        .iter()
        .find(|t| t.id == "t7-p0-gates")
        .unwrap();
    assert!(
        insp.depends_on.iter().any(|d| d == SYS_CLOSEOUT_ID),
        "inspect must wait on closeout; deps={:?}",
        insp.depends_on
    );
    assert!(
        !insp.title.contains("回写台账"),
        "inspect title must strip dual-duty: {}",
        insp.title
    );

    let checklist = config.runs_dir().join(&run_id).join("plan.checklist.json");
    assert!(
        checklist.is_file(),
        "plan.checklist.json must be written under run_dir"
    );
}

/// E3: docs-only FAIL → maybe_auto_rework starts a new run + writes auto_rework.json.
#[tokio::test]
async fn docs_only_fail_auto_rework_starts_new_run() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/VERDICT.md"),
        "FAIL\nledger/map gaps\n",
    )
    .unwrap();
    std::fs::write(project.join(".cco-out/inspect/ISSUES.md"), DOCS_ISSUES).unwrap();

    let plan_path = project.join("docs/plans/ensure-docs-fail.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: ensure-docs-fail
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
      paths: [docs/**]
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
    config.default.cost_route_enabled = false;
    config.default.cost_escalate_enabled = false;
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.auto_rework = true;
    config.default.auto_rework_docs_only = true;
    // Inject closeout on materialize; for this e2e we use bare scheduler + resolved plan
    // so auto_rework path is isolated from closeout worker timing.
    config.default.auto_closeout = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id.clone(),
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        serde_json::to_string_pretty(&ir).unwrap(),
    )
    .unwrap();

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Paused);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["inspect"].status, TaskStatus::Failed);

    let resp = maybe_auto_rework(&config, &run_id)
        .unwrap()
        .expect("docs-only FAIL must auto-start rework");
    assert_ne!(resp.run_id, run_id);
    assert_eq!(resp.source_run_id, run_id);
    assert_eq!(resp.round, 1);
    assert!(
        config
            .runs_dir()
            .join(&resp.run_id)
            .join("run.json")
            .is_file(),
        "new rework run dir must exist"
    );
    let marker = run_dir.join("auto_rework.json");
    assert!(marker.is_file(), "auto_rework.json marker on source run");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
    assert_eq!(v["auto_rework_run_id"], resp.run_id);
    assert_eq!(v["trigger"], "docs-closeout");
}

/// E3: mixed business blocking under docs_only → no auto rework (stop for human).
#[tokio::test]
async fn business_blocking_docs_only_does_not_auto_rework() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(
        project.join(".cco-out/inspect/VERDICT.md"),
        "FAIL\nengine\n",
    )
    .unwrap();
    std::fs::write(project.join(".cco-out/inspect/ISSUES.md"), BUSINESS_ISSUES).unwrap();

    let plan_path = project.join("docs/plans/ensure-biz-fail.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: ensure-biz-fail
on_failure: pause
retry_max: 0
defaults:
  provider: fake
  mode: print
tasks:
  - id: implement
    title: stub
    role: implement
    scope:
      paths: [src/**]
    prompt: "stub\nCCO_DONE ok"
  - id: inspect
    title: inspect
    role: inspect
    depends_on: [implement]
    outputs:
      - .cco-out/inspect/VERDICT.md
      - .cco-out/inspect/ISSUES.md
    prompt: "inspect\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.default.cost_route_enabled = false;
    config.default.cost_escalate_enabled = false;
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.auto_rework = true;
    config.default.auto_rework_docs_only = true;
    config.default.auto_closeout = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id.clone(),
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        serde_json::to_string_pretty(&ir).unwrap(),
    )
    .unwrap();

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = make_scheduler(ir, run_state, registry).run().await.unwrap();
    assert_eq!(status, RunStatus::Paused);

    let resp = maybe_auto_rework(&config, &run_id).unwrap();
    assert!(
        resp.is_none(),
        "business blocking must not auto-rework under docs_only; got {resp:?}"
    );
    assert!(!run_dir.join("auto_rework.json").is_file());
}

/// Config off: never auto rework even for pure docs FAIL.
#[tokio::test]
async fn auto_rework_off_skips() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    std::fs::create_dir_all(project.join(".cco-out/inspect")).unwrap();
    std::fs::write(project.join(".cco-out/inspect/VERDICT.md"), "FAIL\n").unwrap();
    std::fs::write(project.join(".cco-out/inspect/ISSUES.md"), DOCS_ISSUES).unwrap();

    let plan_path = project.join("docs/plans/ensure-off.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: ensure-off
on_failure: pause
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "a\nCCO_DONE ok"
  - id: inspect
    title: inspect
    role: inspect
    depends_on: [a]
    outputs: [.cco-out/inspect/VERDICT.md, .cco-out/inspect/ISSUES.md]
    prompt: "i\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.default.cost_route_enabled = false;
    config.default.cost_escalate_enabled = false;
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.auto_rework = false;
    config.default.auto_rework_docs_only = true;
    config.default.auto_closeout = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id.clone(),
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        serde_json::to_string_pretty(&ir).unwrap(),
    )
    .unwrap();

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let _ = make_scheduler(ir, run_state, registry).run().await.unwrap();

    assert!(maybe_auto_rework(&config, &run_id).unwrap().is_none());
}
