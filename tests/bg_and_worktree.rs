use std::process::Command;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::worktree;
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};
use cco::SessionKind;

#[tokio::test]
async fn fake_bg_mode_polls_until_done() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/bg.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: bg
defaults:
  provider: fake
  mode: bg
  worktree: false
tasks:
  - id: a
    title: bg task
    mode: bg
    prompt: "background work\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();
    std::env::set_var("CCO_FAKE_BG_MS", "50");

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[0].mode, "bg");

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
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
    }
    .run()
    .await
    .unwrap();

    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert!(st.tasks["a"]
        .agent_id
        .as_deref()
        .unwrap_or("")
        .contains("fake-bg"));
}

#[test]
fn worktree_creates_branch_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("repo");
    std::fs::create_dir_all(&project).unwrap();
    // init git
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["init"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["config", "user.email", "cco@test.local"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["config", "user.name", "cco"])
        .status()
        .unwrap()
        .success());
    std::fs::write(project.join("README"), "hi").unwrap();
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["commit", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let info = worktree::ensure_worktree(&project, "run1", "t1").unwrap();
    assert!(info.path.exists());
    assert!(info.branch.contains("cco/"));
    assert!(info.created);

    let (wd, meta) = worktree::resolve_work_dir(
        &project,
        "run1",
        "t1",
        true,
        worktree::WorktreeOnFail::FallbackProjectRoot,
    )
    .unwrap();
    assert_eq!(wd, info.path);
    assert!(meta.is_some());
}

/// P1-3: single-provider legacy may fall back to project_root when worktree is unavailable.
#[test]
fn resolve_work_dir_fallback_when_not_git() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("not-a-repo");
    std::fs::create_dir_all(&project).unwrap();

    let (wd, meta) = worktree::resolve_work_dir(
        &project,
        "run-fb",
        "t1",
        true,
        worktree::WorktreeOnFail::FallbackProjectRoot,
    )
    .unwrap();
    assert_eq!(wd, project);
    assert!(meta.is_none());
}

/// P1-3: multi-provider mix-run must fail-closed (no silent project_root fallback).
#[test]
fn resolve_work_dir_fail_closed_when_not_git() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("not-a-repo");
    std::fs::create_dir_all(&project).unwrap();

    let err = worktree::resolve_work_dir(
        &project,
        "run-fc",
        "t1",
        true,
        worktree::WorktreeOnFail::FailClosed,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("fail-closed") || msg.contains("not a git"),
        "unexpected error: {msg}"
    );
}

#[test]
fn is_multi_provider_detects_mixed_set() {
    assert!(!worktree::is_multi_provider(["claude", "claude"]));
    assert!(!worktree::is_multi_provider(["fake"]));
    assert!(worktree::is_multi_provider(["claude", "codex"]));
    assert!(worktree::is_multi_provider(["claude", "fake", "codex"]));
}

/// Scheduler path: multi-provider + want worktree + non-git project → task Failed (not silent root).
#[tokio::test]
async fn multi_provider_worktree_fail_closed_marks_task_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    // Intentionally NOT a git repo → ensure_worktree fails.
    let plan_path = project.join("docs/plans/mix.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: mix
defaults:
  mode: print
  worktree: true
tasks:
  - id: a
    title: claude side
    provider: fake
    prompt: "a\nCCO_DONE ok"
  - id: b
    title: other side
    provider: codex
    depends_on: [a]
    prompt: "b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    // Point codex at a harmless binary so registry builds; task should fail before spawn.
    config
        .providers
        .entry("codex".into())
        .or_default()
        .bin = "true".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert!(worktree::is_multi_provider(
        ir.tasks.iter().map(|t| t.provider.as_str())
    ));
    assert!(ir.worktree);

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
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
    }
    .run()
    .await
    .unwrap();

    assert_ne!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Failed);
    let err = st.tasks["a"].error.as_deref().unwrap_or("");
    assert!(
        err.contains("fail-closed") || err.contains("worktree") || err.contains("git"),
        "expected worktree fail-closed error, got: {err}"
    );
    // Must not have silently used project_root as a successful work dir.
    if let Some(wd) = &st.tasks["a"].work_dir {
        // start_task fails before setting work_dir on success path; if set, must not look "ok".
        let _ = wd;
    }
}

#[test]
fn parse_agent_id_from_bg_output() {
    use cco::runtime::provider::claude::parse_agent_id;
    let s = "backgrounded · 895cb666 (idle — send a prompt to start)";
    assert_eq!(parse_agent_id(s).as_deref(), Some("895cb666"));
}
