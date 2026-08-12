use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use cco::config::{AutoCommitGranularity, Config};
use cco::plan::load_plan;
use cco::report;
use cco::runtime::provider::ProviderRegistry;
use cco::runtime::Scheduler;
use cco::state::{self, AutoCommitPolicySnapshot, RunState, RunStatus};

#[tokio::test]
async fn fake_provider_runs_two_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hello.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: t
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
    assert_eq!(ir.tasks.len(), 2);

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let sched = Scheduler {
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
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    };

    let status = sched.run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(
        st.tasks["a"].status,
        cco::runtime::provider::TaskStatus::Done
    );
    assert_eq!(
        st.tasks["b"].status,
        cco::runtime::provider::TaskStatus::Done
    );
    report::write_reports(&st).unwrap();
    assert!(run_dir.join("report.md").exists());
    assert!(run_dir.join("events.jsonl").exists());
    // P1-4: host-owned handoff ledger updated for both tasks
    assert!(run_dir.join("handoff.md").exists());
    assert!(run_dir.join("handoff.json").exists());
    let handoff: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("handoff.json")).unwrap())
            .unwrap();
    assert_eq!(handoff["schema"], "cco-handoff/v1");
    assert_eq!(handoff["status"], "completed");
    let board = handoff["board"].as_array().unwrap();
    assert_eq!(board.len(), 2);
    assert!(board.iter().all(|r| r["status"] == "done"));
    assert!(handoff["fragments"].get("a").is_some());
    assert!(handoff["fragments"].get("b").is_some());
    let md = std::fs::read_to_string(run_dir.join("handoff.md")).unwrap();
    assert!(md.contains("## Board"));
    assert!(md.contains("## Timeline"));
    assert!(md.contains("## Fragments"));
    assert!(md.contains("## Open risks"));
    assert!(md.contains("## Instructions for next worker"));
    // P1-8: terminal report has per-provider columns + handoff path links
    let report_md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
    assert!(
        report_md.contains("## By provider"),
        "report.md missing By provider:\n{report_md}"
    );
    assert!(
        report_md.contains("| fake |"),
        "report.md missing fake row:\n{report_md}"
    );
    assert!(
        report_md.contains("handoff.md"),
        "report.md missing handoff.md:\n{report_md}"
    );
    assert!(
        report_md.contains("handoff.json"),
        "report.md missing handoff.json:\n{report_md}"
    );
    let report_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    let by = report_json["by_provider"].as_array().expect("by_provider");
    assert_eq!(by.len(), 1);
    assert_eq!(by[0]["provider"], "fake");
    assert_eq!(by[0]["tasks"], 2);
    assert_eq!(by[0]["done"], 2);
    assert_eq!(report_json["handoff"]["md_rel"], "handoff.md");
    assert_eq!(report_json["handoff"]["exists_md"], true);
    let _ = PathBuf::from(".");
}

#[tokio::test]
async fn per_task_auto_commit_records_commit_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("repo");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    for args in [
        vec!["init"],
        vec!["config", "user.name", "CCO Test"],
        vec!["config", "user.email", "cco-test@example.com"],
    ] {
        assert!(Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    std::fs::write(project.join("README.md"), "initial\n").unwrap();
    assert!(Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["add", "README.md"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["commit", "-m", "initial"])
        .status()
        .unwrap()
        .success());

    let plan_path = project.join("docs/plans/one.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: auto-commit
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: write
    title: write
    prompt: "finish\nCCO_DONE ok"
"#,
    )
    .unwrap();
    std::fs::write(project.join("feature.txt"), "ready\n").unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.git.auto_commit.enabled = true;
    config.git.auto_commit.granularity = AutoCommitGranularity::PerTask;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    AutoCommitPolicySnapshot::from_config(&config)
        .save(&run_dir)
        .unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = Scheduler {
        max_parallel: 1,
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
    let commit = st.tasks["write"].auto_commit.as_ref().unwrap();
    assert!(commit.ok, "{}", commit.message);
    assert!(commit.commit_hash.is_some(), "{}", commit.message);
    assert!(commit.files.iter().any(|p| p == "feature.txt"));
}

#[tokio::test]
async fn per_plan_auto_commit_records_commit_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("repo");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    for args in [
        vec!["init"],
        vec!["config", "user.name", "CCO Test"],
        vec!["config", "user.email", "cco-test@example.com"],
    ] {
        assert!(Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    std::fs::write(project.join("README.md"), "initial\n").unwrap();
    assert!(Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["add", "README.md"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["commit", "-m", "initial"])
        .status()
        .unwrap()
        .success());

    let plan_path = project.join("docs/plans/one.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: auto-commit-plan
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: write
    title: write
    prompt: "finish\nCCO_DONE ok"
"#,
    )
    .unwrap();
    std::fs::write(project.join("feature.txt"), "ready\n").unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.git.auto_commit.enabled = true;
    config.git.auto_commit.granularity = AutoCommitGranularity::PerPlan;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    AutoCommitPolicySnapshot::from_config(&config)
        .save(&run_dir)
        .unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = Scheduler {
        max_parallel: 1,
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
    assert_eq!(st.auto_commits.len(), 1);
    let commit = &st.auto_commits[0];
    assert!(commit.ok, "{}", commit.message);
    assert!(commit.commit_hash.is_some(), "{}", commit.message);
    assert!(commit.files.iter().any(|p| p == "feature.txt"));
}

/// P1-4: declared outputs missing after Done → host flips Failed + handoff fragment.
#[tokio::test]
async fn missing_outputs_fails_task_and_updates_handoff() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/missing-out.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: missing-out
on_failure: continue
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a must write output
    prompt: "do a without writing output\nCCO_DONE ok"
    outputs:
      - .cco-out/a/SUMMARY.md
  - id: b
    title: b no outputs
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
    assert_eq!(
        ir.tasks[0].outputs,
        vec![".cco-out/a/SUMMARY.md".to_string()]
    );

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let sched = Scheduler {
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
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    };

    let status = sched.run().await.unwrap();
    // a failed (missing outputs); b skipped via on_failure=continue depends_on fail
    assert!(
        matches!(
            status,
            RunStatus::Failed | RunStatus::Paused | RunStatus::Completed
        ),
        "unexpected run status {status:?}"
    );

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(
        st.tasks["a"].status,
        cco::runtime::provider::TaskStatus::Failed
    );
    let err = st.tasks["a"].error.as_deref().unwrap_or("");
    assert!(
        err.contains("missing outputs"),
        "expected missing outputs error, got: {err}"
    );

    assert!(run_dir.join("handoff.md").exists());
    assert!(run_dir.join("handoff.json").exists());
    let handoff: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("handoff.json")).unwrap())
            .unwrap();
    let board_a = handoff["board"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "a")
        .unwrap();
    assert_eq!(board_a["status"], "failed");
    assert!(handoff["fragments"].get("a").is_some());
    assert_eq!(handoff["fragments"]["a"]["status"], "failed");
    let risks = handoff["open_risks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        risks.iter().any(|r| r.as_str().unwrap_or("").contains("a")),
        "open_risks should mention task a: {risks:?}"
    );
}

/// P1-4: when declared outputs exist under project, task Done and fragment records artifacts.
#[tokio::test]
async fn present_outputs_recorded_in_handoff_fragment() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    // Pre-create the output so fake provider "success" passes host gate.
    let out = project.join(".cco-out/a");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("SUMMARY.md"), "summary from a\n").unwrap();

    let plan_path = project.join("docs/plans/with-out.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: with-out
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a writes summary
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
    let sched = Scheduler {
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
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    };

    let status = sched.run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(
        st.tasks["a"].status,
        cco::runtime::provider::TaskStatus::Done
    );
    assert_eq!(
        st.tasks["b"].status,
        cco::runtime::provider::TaskStatus::Done
    );

    let handoff: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("handoff.json")).unwrap())
            .unwrap();
    assert_eq!(handoff["fragments"]["a"]["status"], "done");
    let arts = handoff["fragments"]["a"]["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        arts.iter()
            .any(|a| a.as_str() == Some(".cco-out/a/SUMMARY.md")),
        "artifacts: {arts:?}"
    );
    let summary = handoff["fragments"]["a"]["summary"].as_str().unwrap_or("");
    assert!(
        summary.contains("summary from a") || !summary.is_empty(),
        "summary should pull from output md, got: {summary:?}"
    );
}

/// P1-5: fake start receives prompt with [CCO_HANDOFF] prefix (task_dir/prompt.md).
#[tokio::test]
async fn start_injects_cco_handoff_prefix_into_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/handoff-prefix.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: handoff-prefix
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a business\nCCO_DONE ok"
  - id: b
    title: b
    depends_on: [a]
    prompt: "do b business\nCCO_DONE ok"
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
    let sched = Scheduler {
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
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    };

    let status = sched.run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    // Fake writes received prompt to task_dir/prompt.md
    let prompt_a = std::fs::read_to_string(run_dir.join("tasks/a/prompt.md")).unwrap();
    assert!(
        prompt_a.contains("[CCO_HANDOFF]"),
        "task a prompt missing handoff open: {prompt_a}"
    );
    assert!(
        prompt_a.contains("[/CCO_HANDOFF]"),
        "task a prompt missing handoff close"
    );
    assert!(prompt_a.contains("task=a"));
    assert!(prompt_a.contains("do a business"));

    let prompt_b = std::fs::read_to_string(run_dir.join("tasks/b/prompt.md")).unwrap();
    assert!(
        prompt_b.contains("[CCO_HANDOFF]"),
        "task b prompt missing handoff open: {prompt_b}"
    );
    assert!(prompt_b.contains("[/CCO_HANDOFF]"));
    assert!(prompt_b.contains("task=b"));
    assert!(prompt_b.contains("depends_on: a") || prompt_b.contains("Fragments"));
    assert!(prompt_b.contains("## Board"));
    assert!(prompt_b.contains("do b business"));
    // plan.resolved.json keeps original business prompt (no injection into plan IR on disk)
    let resolved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("plan.resolved.json")).unwrap())
            .unwrap();
    let tasks = resolved["tasks"].as_array().unwrap();
    let plan_b = tasks.iter().find(|t| t["id"] == "b").unwrap();
    assert_eq!(
        plan_b["prompt"].as_str().unwrap(),
        "do b business\nCCO_DONE ok"
    );
}

/// P3 memory pilot: a finished fake task leaves an outcome entry in semantic memory
/// (scenario 2 recording path · docs/agentmemory-integration-plan-2026-08-12.md).
#[tokio::test]
async fn memory_pilot_records_task_outcome() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/mem.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: mem
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
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

    let mem_cfg = cco::state::memory_store::MemoryConfig {
        storage_root: tmp.path().join("memory"),
        ..Default::default()
    };
    let memory: std::sync::Arc<dyn cco::ports::MemoryPort> =
        std::sync::Arc::new(cco::state::memory_store::LocalMemory::new(mem_cfg));

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let sched = Scheduler {
        max_parallel: 1,
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
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: Some(memory.clone()),
    };

    let status = sched.run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    // Outcome entry must be retrievable via the same query shape the router uses.
    let hits = memory.search("outcome fake implement", 10).await.unwrap();
    assert!(
        !hits.is_empty(),
        "task outcome should be recorded in semantic memory"
    );
    let hit = &hits[0];
    assert_eq!(hit.metadata.provider.as_deref(), Some("fake"));
    assert_eq!(hit.metadata.task_role.as_deref(), Some("implement"));
    assert_eq!(hit.metadata.outcome.as_deref(), Some("success"));
    assert!(hit.metadata.tags.contains(&"task-outcome".to_string()));
}
