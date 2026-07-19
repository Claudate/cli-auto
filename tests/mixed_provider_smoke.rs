//! P0-5 + P1 success-criteria smoke (fake / mock registry).
//!
//! - Same-run events contain task_start for different providers.
//! - Legal multi-provider plan writes handoff after terminal tasks.
//! - Illegal mix: multi-provider parallel + worktree=false → validate fails.
//! - Illegal: codex + mode=bg → validate fails.
//! - Optional: real claude/codex bins are skipped when absent (no account required).

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::handoff::Handoff;
use cco::runtime::provider::fake::FakeProvider;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};

fn mixed_registry() -> ProviderRegistry {
    ProviderRegistry::from_providers(vec![
        Arc::new(FakeProvider::with_name("inline".into(), "claude")),
        Arc::new(FakeProvider::with_name("inline".into(), "codex")),
        Arc::new(FakeProvider::new("inline".into())),
    ])
    .expect("mixed fake registry")
}

fn make_scheduler(
    ir: cco::plan::PlanIR,
    run_state: RunState,
    registry: ProviderRegistry,
    max_parallel: usize,
) -> Scheduler {
    Scheduler {
        max_parallel,
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

/// P0-5: serial multi-provider (claude → codex) via fake aliases; events + state + handoff.
#[tokio::test]
async fn mixed_serial_task_start_events_and_handoff() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/mixed-serial.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: mixed-serial
defaults:
  mode: print
  worktree: false
tasks:
  - id: feat-a
    title: claude implement
    provider: claude
    role: implement
    scope:
      paths: [src/a/**, .cco-out/a/**]
    prompt: "do a\nCCO_DONE ok"
  - id: feat-b
    title: codex implement
    provider: codex
    role: implement
    depends_on: [feat-a]
    scope:
      paths: [src/b/**, .cco-out/b/**]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "claude".into();
    config.default.worktree = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[0].provider, "claude");
    assert_eq!(ir.tasks[1].provider, "codex");

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let status = make_scheduler(ir, run_state, mixed_registry(), 2)
        .run()
        .await
        .unwrap();
    assert_eq!(status, RunStatus::Completed);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["feat-a"].status, TaskStatus::Done);
    assert_eq!(st.tasks["feat-b"].status, TaskStatus::Done);
    assert_eq!(st.tasks["feat-a"].provider, "claude");
    assert_eq!(st.tasks["feat-b"].provider, "codex");

    let events = read_events(&run_dir);
    let starts: Vec<_> = events
        .iter()
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("task_start"))
        .collect();
    assert!(
        starts.len() >= 2,
        "expected ≥2 task_start events, got {} full={events:?}",
        starts.len()
    );
    let providers: std::collections::HashSet<&str> = starts
        .iter()
        .filter_map(|e| e.get("provider").and_then(|v| v.as_str()))
        .collect();
    assert!(
        providers.contains("claude") && providers.contains("codex"),
        "task_start providers must include claude and codex, got {providers:?} starts={starts:?}"
    );

    // Handoff ledger after terminal tasks
    assert!(run_dir.join("handoff.md").exists());
    assert!(run_dir.join("handoff.json").exists());
    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.status, "completed");
    assert_eq!(h.board.len(), 2);
    assert!(h.board.iter().all(|r| r.status == "done"));
    assert_eq!(h.fragments["feat-a"].provider, "claude");
    assert_eq!(h.fragments["feat-b"].provider, "codex");

    // Start-inject: second task prompt carries handoff prefix
    let prompt_b = std::fs::read_to_string(run_dir.join("tasks/feat-b/prompt.md")).unwrap();
    assert!(
        prompt_b.contains("[CCO_HANDOFF]"),
        "feat-b prompt missing handoff prefix: {prompt_b}"
    );
}

/// P0-5 parallel wave: multi-provider + worktree on a real git project (fake aliases).
#[tokio::test]
async fn mixed_parallel_task_start_with_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    // git init so worktree fail-closed path is not taken
    let st = Command::new("git")
        .args(["init"])
        .current_dir(&project)
        .status();
    match st {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("skip mixed_parallel_task_start_with_worktree: git init failed");
            return;
        }
    }
    let _ = Command::new("git")
        .args(["config", "user.email", "cco-test@example.com"])
        .current_dir(&project)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.name", "cco-test"])
        .current_dir(&project)
        .status();
    std::fs::write(project.join("README.md"), "seed\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&project)
        .status();
    let commit = Command::new("git")
        .args(["commit", "-m", "seed"])
        .current_dir(&project)
        .status();
    if !commit.map(|s| s.success()).unwrap_or(false) {
        eprintln!("skip mixed_parallel: git commit failed");
        return;
    }

    let plan_path = project.join("docs/plans/mixed-parallel.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: mixed-parallel
defaults:
  mode: print
  worktree: true
max_parallel: 2
tasks:
  - id: feat-a
    title: claude side
    provider: claude
    role: implement
    scope:
      paths: [src/a/**, .cco-out/a/**]
    prompt: "do a\nCCO_DONE ok"
  - id: feat-b
    title: codex side
    provider: codex
    role: implement
    scope:
      paths: [src/b/**, .cco-out/b/**]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "claude".into();
    config.default.worktree = true;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    ir.validate().expect("legal parallel mixed plan");

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let status = make_scheduler(ir, run_state, mixed_registry(), 2)
        .run()
        .await
        .unwrap();
    assert_eq!(status, RunStatus::Completed, "parallel mixed run should complete");

    let events = read_events(&run_dir);
    let starts: Vec<_> = events
        .iter()
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("task_start"))
        .collect();
    let providers: std::collections::HashSet<&str> = starts
        .iter()
        .filter_map(|e| e.get("provider").and_then(|v| v.as_str()))
        .collect();
    assert!(
        providers.contains("claude") && providers.contains("codex"),
        "parallel task_start must show both providers: {providers:?}"
    );

    let h = Handoff::load(&run_dir).unwrap();
    assert!(h.fragments.contains_key("feat-a"));
    assert!(h.fragments.contains_key("feat-b"));
    assert_eq!(h.fragments["feat-a"].provider, "claude");
    assert_eq!(h.fragments["feat-b"].provider, "codex");
}

/// P1: multi-provider parallel + worktree=false → load/validate fails.
#[test]
fn validate_rejects_mixed_parallel_without_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/illegal-wt.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: illegal-wt
defaults:
  mode: print
  worktree: false
tasks:
  - id: a
    title: a
    provider: claude
    prompt: "a\nCCO_DONE ok"
  - id: b
    title: b
    provider: codex
    prompt: "b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.worktree = false;
    let err = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("worktree") || err.contains("multi-provider") || err.contains("parallel"),
        "expected worktree multi-provider error, got: {err}"
    );
}

/// P1: codex + mode=bg → validate fails.
#[test]
fn validate_rejects_codex_bg() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/illegal-bg.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: illegal-bg
defaults:
  mode: print
  worktree: false
tasks:
  - id: c
    title: codex bg
    provider: codex
    mode: bg
    prompt: "bg not allowed\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    let err = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config)
        .unwrap_err()
        .to_string();
    assert!(err.contains("codex"), "expected codex in error: {err}");
    assert!(err.contains("bg"), "expected bg in error: {err}");
}

/// Real CLI smoke is skipped when binaries are missing (no account / install required).
#[test]
fn real_cli_bins_optional_skip() {
    let claude = which::which("claude").ok().or_else(|| {
        std::env::var_os("CCO_CLAUDE_BIN").map(std::path::PathBuf::from)
    });
    let codex = which::which("codex").ok().or_else(|| {
        std::env::var_os("CCO_CODEX_BIN").map(std::path::PathBuf::from)
    });
    if claude.is_none() || codex.is_none() {
        eprintln!(
            "skip real multi-cli smoke: claude={claude:?} codex={codex:?} \
             (fake-path coverage is sufficient for P0-5 / P1)"
        );
        return;
    }
    // Presence only — do not spawn real sessions (account / cost).
    eprintln!(
        "real bins present (not spawning): claude={} codex={}",
        claude.unwrap().display(),
        codex.unwrap().display()
    );
}
