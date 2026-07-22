//! Integration-style unit tests for write_reports (P0-2 / P0-3).

use super::*;
use crate::runtime::provider::TaskStatus;
use crate::state::{RunState, RunStatus, TaskState};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;

fn task(provider: &str, status: TaskStatus, cost: Option<f64>) -> TaskState {
    let mut t = TaskState::pending(provider, "print");
    t.status = status;
    t.cost_usd = cost;
    t
}

#[test]
fn summarize_providers_counts_and_cost() {
    let tasks = vec![
        task("claude", TaskStatus::Done, Some(0.10)),
        task("claude", TaskStatus::Running, None),
        task("claude", TaskStatus::Failed, Some(0.05)),
        task("codex", TaskStatus::Done, Some(1.25)),
        task("codex", TaskStatus::Pending, None),
        task("fake", TaskStatus::Skipped, None),
    ];
    let rows = summarize_providers(tasks.iter());
    assert_eq!(rows.len(), 3);
    let claude = rows.iter().find(|r| r.provider == "claude").unwrap();
    assert_eq!(claude.tasks, 3);
    assert_eq!(claude.running, 1);
    assert_eq!(claude.done, 1);
    assert_eq!(claude.failed, 1);
    assert!((claude.cost_usd.unwrap() - 0.15).abs() < 1e-9);
    let codex = rows.iter().find(|r| r.provider == "codex").unwrap();
    assert_eq!(codex.tasks, 2);
    assert_eq!(codex.done, 1);
    assert_eq!(codex.pending, 1);
    assert!((codex.cost_usd.unwrap() - 1.25).abs() < 1e-9);
    let fake = rows.iter().find(|r| r.provider == "fake").unwrap();
    assert_eq!(fake.other, 1);
    assert!(fake.cost_usd.is_none());
}

#[test]
fn write_reports_includes_by_provider_and_handoff_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let run_dir = tmp.path().join("run-p1-8");
    std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
    // Pretend mid-run handoff already exists (host ledger).
    std::fs::write(run_dir.join("handoff.md"), "# handoff\n").unwrap();
    std::fs::write(run_dir.join("handoff.json"), r#"{"schema":"cco-handoff/v1"}"#).unwrap();

    let mut tasks = HashMap::new();
    tasks.insert("a".into(), task("claude", TaskStatus::Done, Some(0.2)));
    tasks.insert("b".into(), task("codex", TaskStatus::Failed, Some(0.01)));
    tasks.insert("c".into(), task("claude", TaskStatus::Running, None));

    let state = RunState {
        schema: "cco-run/v1".into(),
        run_id: "run-p1-8".into(),
        project_root: tmp.path().join("proj"),
        plan_path: tmp.path().join("plan.yaml"),
        adapter: "cco-plan/v1".into(),
        started_at: Utc::now(),
        finished_at: None,
        status: RunStatus::Running,
        tasks,
        run_dir: run_dir.clone(),
    };
    write_reports(&state).unwrap();

    let md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
    assert!(
        md.contains("### By provider") || md.contains("## By provider"),
        "missing by-provider section:\n{md}"
    );
    assert!(md.contains("| claude |"), "missing claude row:\n{md}");
    assert!(md.contains("| codex |"), "missing codex row:\n{md}");
    assert!(md.contains("handoff.md"), "missing handoff.md link:\n{md}");
    assert!(md.contains("handoff.json"), "missing handoff.json link:\n{md}");
    assert!(md.contains("## 对照计划"), "P0-3 skeleton missing 对照计划:\n{md}");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    let by = json["by_provider"].as_array().expect("by_provider array");
    assert_eq!(by.len(), 2);
    let claude = by.iter().find(|r| r["provider"] == "claude").unwrap();
    assert_eq!(claude["tasks"], 2);
    assert_eq!(claude["running"], 1);
    assert_eq!(claude["done"], 1);
    assert!((claude["cost_usd"].as_f64().unwrap() - 0.2).abs() < 1e-9);
    let codex = by.iter().find(|r| r["provider"] == "codex").unwrap();
    assert_eq!(codex["failed"], 1);
    assert_eq!(json["handoff"]["md_rel"], "handoff.md");
    assert_eq!(json["handoff"]["json_rel"], "handoff.json");
    assert_eq!(json["handoff"]["exists_md"], true);
    assert!(
        json["handoff"]["md"]
            .as_str()
            .unwrap()
            .ends_with("handoff.md")
    );

    let status_txt = format_status_by_provider(&state.tasks);
    assert!(status_txt.contains("claude:"));
    assert!(status_txt.contains("running=1"));
    assert!(status_txt.contains("codex:"));
}

#[test]
fn plan_short_name_from_path() {
    assert_eq!(
        plan_short_name(Path::new("docs/pilotdeck-borrow-landing-2026-07-21.md")),
        "pilotdeck-borrow-landing-2026-07-21"
    );
    assert_eq!(plan_short_name(Path::new("plan.yaml")), "plan");
    assert_eq!(plan_short_name(Path::new("")), "未命名计划");
}

#[test]
fn write_reports_human_headline_and_notes_sink() {
    let tmp = tempfile::tempdir().unwrap();
    let run_dir = tmp.path().join("run-p0-2");
    std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
    std::fs::write(run_dir.join("handoff.md"), "# handoff\n").unwrap();

    let mut tasks = HashMap::new();
    tasks.insert("a".into(), task("claude", TaskStatus::Done, Some(0.2)));
    tasks.insert("b".into(), task("codex", TaskStatus::Failed, None));
    tasks.insert("c".into(), task("claude", TaskStatus::Done, Some(0.1)));

    let plan = tmp.path().join("docs/my-cool-plan.md");
    let state = RunState {
        schema: "cco-run/v1".into(),
        run_id: "20260722T003715Z-5ac8".into(),
        project_root: tmp.path().join("proj"),
        plan_path: plan.clone(),
        adapter: "cco-plan/v1".into(),
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        status: RunStatus::Completed,
        tasks,
        run_dir: run_dir.clone(),
    };
    write_reports(&state).unwrap();

    let md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
    let first = md.lines().next().unwrap_or("");
    assert_eq!(
        first, "# 本轮结果 · 《my-cool-plan》",
        "H1 must be human plan short name, got: {first}"
    );
    // First line must not embed run_id (acceptance).
    assert!(
        !first.contains("20260722T003715Z-5ac8"),
        "run_id leaked into H1:\n{first}"
    );
    assert!(
        !first.contains("cco report"),
        "old machine title still present:\n{first}"
    );
    assert!(
        md.contains("本轮状态：**已完成** · 完成 2/3 项任务"),
        "missing human summary:\n{md}"
    );
    assert!(
        md.contains("## 花费与用时"),
        "cost/elapsed section must be present:\n{md}"
    );
    assert!(md.contains("## 备注"), "metadata must sink to ## 备注:\n{md}");
    assert!(md.contains("## 对照计划"), "P0-3 对照计划 section:\n{md}");
    assert!(md.contains("## 步骤结果"), "P0-3 步骤结果 section:\n{md}");
    assert!(md.contains("## 后续"), "P0-3 后续 section:\n{md}");
    assert!(
        md.contains("**run_id**: `20260722T003715Z-5ac8`"),
        "run_id should appear under 备注:\n{md}"
    );
    assert!(
        md.contains("**adapter**: cco-plan/v1"),
        "adapter should appear under 备注:\n{md}"
    );
    // Absolute paths only in 备注, not before it as primary narrative.
    let notes_idx = md.find("## 备注").expect("备注 section");
    let before_notes = &md[..notes_idx];
    assert!(
        !before_notes.contains("20260722T003715Z-5ac8"),
        "run_id must not appear before 备注:\n{before_notes}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    assert_eq!(json["headline"], "本轮结果 · 《my-cool-plan》");
    // Old fields still present (compat).
    assert_eq!(json["run_id"], "20260722T003715Z-5ac8");
    assert_eq!(json["adapter"], "cco-plan/v1");
    assert_eq!(json["tasks_done"], 2);
    assert_eq!(json["tasks_total"], 3);
}

#[test]
fn handoff_paths_relative_and_abs() {
    let p = PathBuf::from("/tmp/fake-run");
    let h = handoff_paths(&p);
    assert_eq!(h.md_rel, "handoff.md");
    assert_eq!(h.json_rel, "handoff.json");
    assert!(h.md.ends_with("handoff.md"));
    assert!(h.json.ends_with("handoff.json"));
}

/// P0-3: no handoff → still full skeleton; 对照计划 is placeholder; Notes has fallback.
#[test]
fn write_reports_fallback_without_handoff() {
    let tmp = tempfile::tempdir().unwrap();
    let run_dir = tmp.path().join("run-p0-3-fb");
    std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
    // deliberately no handoff.md / handoff.json

    let mut tasks = HashMap::new();
    tasks.insert("a".into(), task("claude", TaskStatus::Done, None));
    tasks.insert("b".into(), task("codex", TaskStatus::Done, None));

    let state = RunState {
        schema: "cco-run/v1".into(),
        run_id: "run-p0-3-fb".into(),
        project_root: tmp.path().join("proj"),
        plan_path: tmp.path().join("docs/demo-plan.md"),
        adapter: "cco-plan/v1".into(),
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        status: RunStatus::Completed,
        tasks,
        run_dir: run_dir.clone(),
    };
    write_reports(&state).unwrap();

    let md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
    for heading in [
        "## 对照计划",
        "## 步骤结果",
        "## 花费与用时",
        "## 后续",
        "## 备注",
    ] {
        assert!(md.contains(heading), "missing {heading}:\n{md}");
    }
    assert!(
        md.contains("未开启对照计划巡检") || md.contains("本轮未产出巡检结论"),
        "expected human placeholder in 对照计划:\n{md}"
    );
    // Never invent PASS on fallback path.
    let compare_idx = md.find("## 对照计划").unwrap();
    let next = md[compare_idx + 1..]
        .find("\n## ")
        .map(|i| compare_idx + 1 + i)
        .unwrap_or(md.len());
    let compare_body = &md[compare_idx..next];
    assert!(
        !compare_body.contains("通过") || compare_body.contains("未"),
        "fallback must not claim 通过:\n{compare_body}"
    );
    assert!(
        !compare_body.contains("PASS"),
        "fallback must never write PASS:\n{compare_body}"
    );
    assert!(
        md.contains("**fallback**:") || md.contains("fallback"),
        "Notes must record fallback reason:\n{md}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    assert_eq!(json["plan_compare"]["is_fallback"], true);
    assert!(json["plan_compare"]["fallback_reason"].is_string());
    assert_ne!(
        json["plan_compare"]["kind"].as_str().unwrap_or(""),
        "pass",
        "fallback kind must not be pass"
    );
}

/// P0-3: mock inspect FAIL products → 对照计划 is real (not placeholder) with issues.
#[test]
fn write_reports_with_mock_inspect_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("proj");
    let run_dir = tmp.path().join("run-p0-3-fail");
    std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
    std::fs::create_dir_all(proj.join(".cco-out/inspect")).unwrap();
    std::fs::write(run_dir.join("handoff.md"), "# handoff\n").unwrap();
    std::fs::write(
        proj.join(".cco-out/inspect/VERDICT.md"),
        "Result: FAIL — missing plan-compare section\n",
    )
    .unwrap();
    std::fs::write(
        proj.join(".cco-out/inspect/ISSUES.md"),
        r#"- id: I-1
  severity: blocking
  plan_ref: P0-3
  path: src/report/mod.rs
  symptom: no 对照计划 section when inspect off
  fix_wp: always write plan-compare skeleton
"#,
    )
    .unwrap();
    // require_inspect via plan.resolved.json so view knows gate was on
    std::fs::write(
        run_dir.join("plan.resolved.json"),
        r#"{
  "schema": "cco-plan/v1",
  "name": "demo",
  "adapter": "cco-plan/v1",
  "source_path": "plan.md",
  "max_parallel": 1,
  "on_failure": "pause",
  "retry_max": 0,
  "default_provider": "claude",
  "default_mode": "print",
  "worktree": false,
  "require_inspect": true,
  "tasks": [
{
  "id": "sys-post-inspect",
  "title": "巡检",
  "depends_on": [],
  "group": null,
  "provider": "claude",
  "mode": "print",
  "prompt": "inspect",
  "acceptance": null,
  "timeout_secs": null,
  "worktree": null,
  "provider_opts": {},
  "role": "inspect",
  "outputs": [".cco-out/inspect/VERDICT.md", ".cco-out/inspect/ISSUES.md"]
}
  ]
}"#,
    )
    .unwrap();

    let mut tasks = HashMap::new();
    tasks.insert(
        "sys-post-inspect".into(),
        task("claude", TaskStatus::Done, Some(0.05)),
    );

    let state = RunState {
        schema: "cco-run/v1".into(),
        run_id: "run-p0-3-fail".into(),
        project_root: proj,
        plan_path: tmp.path().join("plan.md"),
        adapter: "cco-plan/v1".into(),
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        status: RunStatus::Failed,
        tasks,
        run_dir: run_dir.clone(),
    };
    write_reports(&state).unwrap();

    let md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
    assert!(md.contains("## 对照计划"), "missing 对照计划:\n{md}");
    assert!(
        md.contains("有遗漏") || md.contains("需处理"),
        "FAIL should surface omissions:\n{md}"
    );
    assert!(
        md.contains("I-1") || md.contains("missing plan-compare") || md.contains("blocking"),
        "issue summary expected:\n{md}"
    );
    // Real path: Notes should not claim pure fallback for PASS invention
    let compare_idx = md.find("## 对照计划").unwrap();
    let next = md[compare_idx + 1..]
        .find("\n## ")
        .map(|i| compare_idx + 1 + i)
        .unwrap_or(md.len());
    let compare_body = &md[compare_idx..next];
    assert!(
        !compare_body.contains("未开启对照计划巡检"),
        "real FAIL must not use disabled placeholder:\n{compare_body}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    assert_eq!(json["plan_compare"]["is_fallback"], false);
    assert_eq!(json["plan_compare"]["kind"], "fail");
    assert!(json["plan_compare"]["blocking_count"].as_u64().unwrap() >= 1);
}
