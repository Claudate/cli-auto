//! Plan job: analyze a plan document into a validated PlanIR before exec.
//!
//! [INPUT]: StartPlanJobRequest · plan 源文件 · config · graph::topo_layers
//! [OUTPUT]: PlanJob/PlanJobView · start_plan_job/get_plan_job/confirm 相关 · plan.proposed.json
//! [POS]: Mode B 规划相位；services/confirm_start 与桌面 planSessions 消费；D4 已目录化
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md
//!
//! Modes:
//! - `parse` — existing adapters (structured / serial-prompts / raw-single)
//! - `fake`  — fixed multi-task DAG for demos without API
//! - `ai`    — LLM planner (print/stream-json → plan JSON) with heuristic fallback
//!
//! Limits: `MAX_TASKS`（validate + heuristic 合并 + LLM 拒绝超限）；stream-json 从
//! 最终 `type=result` 取 plan，勿取 init 事件的首个 `{`。

mod heuristic;
mod job;
mod llm;
mod view;

pub use job::{
    get_plan_job, job_dir, latest_plan_job_for_project, plan_jobs_dir, start_plan_job, PlanJob,
    PlanJobStatus, StartPlanJobRequest,
};
pub use view::{
    job_view, load_proposed, load_proposed_for_exec, mark_confirmed, planner_cost_for_run,
    update_proposed_task, PlanJobView, PlanTaskView,
};

#[cfg(test)]
mod tests {
    use super::heuristic::merge_sections;
    use super::llm::{extract_json_object, parse_llm_plan_output, LlmPlanDoc};
    use super::*;
    use crate::config::Config;
    use crate::plan::MAX_TASKS;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn fake_plan_validates() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# hello\n\ndo something cool\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        assert!(view.task_count.unwrap() >= 3);
        assert!(!view.layers.is_empty());
        assert_eq!(view.layers[0].len(), 2); // t1,t2 parallel
    }

    #[test]
    fn heuristic_splits_headings() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("spec.md");
        std::fs::write(
            &plan,
            "## 准备\n\n写 README\n\n## 功能\n\n实现 foo\n\n## 测试\n\n补测试\n",
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project,
                plan: PathBuf::from("spec.md"),
                plan_mode: Some("ai".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                // Explicit concurrency: 3 sections → one parallel wave.
                max_parallel: Some(3),
            },
        )
        .unwrap();
        // ai mode is async (LLM attempt); poll until planned/failed
        let mut view = view;
        for _ in 0..100 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            view = get_plan_job(&cfg, &view.job_id).unwrap();
        }
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        assert_eq!(view.task_count, Some(3));
        assert_eq!(view.max_parallel, Some(3));
        // Wave barrier: all three independent when max_parallel >= section count.
        assert_eq!(view.layers.len(), 1);
        assert_eq!(view.layers[0].len(), 3);
    }

    #[test]
    fn confirm_starts_run_dir() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(cfg.runs_dir()).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        let run_id = crate::services::confirm_start(cfg.clone(), &view.job_id).unwrap();
        assert!(!run_id.is_empty());
        let job = PlanJob::load(&cfg, &view.job_id).unwrap();
        assert_eq!(job.status, PlanJobStatus::Confirmed);
        assert_eq!(job.run_id.as_deref(), Some(run_id.as_str()));
        // run state file exists
        assert!(cfg.runs_dir().join(&run_id).join("run.json").exists());
        // give scheduler a moment
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    #[test]
    fn latest_job_restores_planned() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\n## a\n\ndo a\n\n## b\n\ndo b\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(4),
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        assert_eq!(view.max_parallel, Some(4));

        let latest = latest_plan_job_for_project(&cfg, &project)
            .unwrap()
            .expect("should find latest");
        assert_eq!(latest.job_id, view.job_id);
        assert_eq!(latest.status, "planned");
        assert!(latest.task_count.unwrap_or(0) >= 1);
        assert!(!latest.tasks.is_empty());
    }

    #[test]
    fn extract_json_from_stream_json_result_not_init() {
        // Repro: first `{` is system init (no tasks). Plan lives in final result.
        let plan_body = r#"{
  "schema": "cco-plan/v1",
  "name": "demo",
  "max_parallel": 2,
  "on_failure": "pause",
  "tasks": [
    {"id": "t1", "title": "A", "depends_on": [], "prompt": "do a\nCCO_DONE ok"},
    {"id": "t2", "title": "B", "depends_on": ["t1"], "prompt": "do b\nCCO_DONE ok"}
  ]
}"#;
        let fenced = format!("```json\n{plan_body}\n```");
        let result_line = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "result": fenced,
            "total_cost_usd": 0.1,
        })
        .to_string();
        let raw = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"system","subtype":"init","cwd":"/tmp","session_id":"x","tools":["Bash"]}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"planning…"}]}}"#,
            result_line
        );
        let extracted = extract_json_object(&raw).expect("extract plan from stream-json");
        let doc: LlmPlanDoc = serde_json::from_str(&extracted).expect("plan deserializes");
        assert_eq!(doc.tasks.len(), 2);
        assert_eq!(doc.tasks[0].id, "t1");
        assert_eq!(doc.name.as_deref(), Some("demo"));
    }

    #[test]
    fn parse_llm_plan_output_stream_json_fixture() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("plan.md");
        std::fs::write(&src, "# idea\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");

        let plan_body = r#"{"schema":"cco-plan/v1","name":"n","max_parallel":1,"tasks":[{"id":"t1","title":"T","depends_on":[],"prompt":"p"}]}"#;
        let result_line = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "result": format!("```json\n{plan_body}\n```"),
        })
        .to_string();
        let raw = format!(
            "{}\n{}\n",
            r#"{"type":"system","subtype":"init","session_id":"s"}"#,
            result_line
        );
        let ir = parse_llm_plan_output(&raw, &src, &cfg).unwrap();
        assert_eq!(ir.tasks.len(), 1);
        assert_eq!(ir.tasks[0].id, "t1");
        ir.validate().unwrap();
    }

    #[test]
    fn merge_sections_caps_at_max() {
        let sections: Vec<_> = (0..35)
            .map(|i| (format!("s{i}"), format!("body {i}")))
            .collect();
        let merged = merge_sections(sections, MAX_TASKS);
        assert_eq!(merged.len(), MAX_TASKS);
        assert!(merged.iter().all(|(_, b)| !b.is_empty()));
    }

    #[test]
    fn heuristic_caps_many_headings() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("big.md");
        let mut md = String::from("# big plan\n\n");
        // 12 ## × 3 ### each = 36 sections if both levels used
        for i in 0..12 {
            md.push_str(&format!("## Chapter {i}\n\nintro {i}\n\n"));
            for j in 0..3 {
                md.push_str(&format!("### Sec {i}.{j}\n\nbody {i}.{j}\n\n"));
            }
        }
        std::fs::write(&plan, &md).unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project,
                plan: PathBuf::from("big.md"),
                plan_mode: Some("ai".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(4),
            },
        )
        .unwrap();
        let mut view = view;
        for _ in 0..100 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            view = get_plan_job(&cfg, &view.job_id).unwrap();
        }
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        let n = view.task_count.unwrap_or(0);
        assert!(n >= 2, "expected multi-task split, got {n}");
        assert!(
            n <= MAX_TASKS,
            "task_count {n} exceeds MAX_TASKS={MAX_TASKS}"
        );
    }

    #[test]
    fn heuristic_spec_doc_not_toc_tasks() {
        use super::heuristic::{build_heuristic_ai_plan, looks_like_spec_document};
        use super::job::PlanJob;

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("multi-cli-collaboration-2026-07-18.md");
        // Minimal chrome-heavy product spec (same failure mode as real multi-cli doc).
        let md = r#"# cco 多 CLI 协作

> 关联真源 · PROTOCOL · GEB 入口 · 不排期则不碰 · D5 池

## 0. 一句话

拆任务时写清 provider。

## 3.5 账本

### Board

| id | provider | role | status | scope | outputs | cost | notes |
|----|----------|------|--------|-------|---------|------|-------|

### Timeline

- step

### Fragments

### task

## 6. 阶段切分与勾选

### P0 — 协议与示例（文档 / 示例为主）

文档

### P1 — host 硬保障（代码）

代码

### P2 — 检验员与分配体验（按需）

体验

## 8. 非目标

N1

## 10. 成功标准

S1

## 12. 修订历史

t1

## 附录 A · 检验员检查清单

1. 完整性
"#;
        std::fs::write(&plan, md).unwrap();
        assert!(
            looks_like_spec_document(md),
            "fixture should classify as product/spec MD"
        );

        // Direct heuristic (no job plumbing): must emit work-order template.
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();
        let now = chrono::Utc::now();
        let job = PlanJob {
            job_id: "plan-test-spec".into(),
            status: PlanJobStatus::Planning,
            project: project.clone(),
            plan_path: PathBuf::from("multi-cli-collaboration-2026-07-18.md"),
            plan_mode: "ai".into(),
            provider: "fake".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: now,
            updated_at: now,
            plan_name: None,
            task_count: None,
            max_parallel: Some(5),
            adapter: None,
            planner_cost_usd: None,
        };
        std::fs::create_dir_all(job_dir(&cfg, &job.job_id)).unwrap();
        let ir = build_heuristic_ai_plan(&cfg, &job).expect("heuristic");
        let titles: Vec<_> = ir.tasks.iter().map(|t| t.title.clone()).collect();
        for bad in [
            "Board",
            "Timeline",
            "Fragments",
            "id | provider",
            "P0",
            "非目标",
            "修订历史",
            "协议与示例",
            "硬保障",
            "raw prompt",
        ] {
            assert!(
                titles.iter().all(|t| !t.contains(bad)),
                "spec split leaked meta title containing {bad:?}: {titles:?}"
            );
        }
        assert!(
            ir.tasks.len() >= 3 && ir.tasks.len() <= 6,
            "expected work-order template size, got {} titles={titles:?}",
            ir.tasks.len()
        );
        // P-loop L0/L1: work-order tail is role=inspect + require_inspect.
        assert!(
            ir.require_inspect,
            "spec work-order should set require_inspect"
        );
        let last = ir.tasks.last().expect("tasks");
        assert_eq!(last.role, Some(crate::plan::TaskRole::Inspect));
        assert!(
            last.outputs.iter().any(|o| o.contains("VERDICT")),
            "inspect outputs missing VERDICT: {:?}",
            last.outputs
        );
        assert!(
            ir.tasks.iter().any(|t| t.prompt.contains("plan_ref")),
            "work-order prompts should require plan_ref"
        );

        // Full Mode B ai path (fake → heuristic, no LLM).
        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project,
                plan: PathBuf::from("multi-cli-collaboration-2026-07-18.md"),
                plan_mode: Some("ai".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(5),
            },
        )
        .unwrap();
        let mut view = view;
        for _ in 0..100 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            view = get_plan_job(&cfg, &view.job_id).unwrap();
        }
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        let proposed = job_dir(&cfg, &view.job_id).join("plan.proposed.json");
        let ir2: crate::plan::PlanIR =
            serde_json::from_str(&std::fs::read_to_string(&proposed).unwrap()).unwrap();
        assert!(
            ir2.tasks.len() >= 3,
            "ai job path should use work-order template, got {} adapter={}",
            ir2.tasks.len(),
            ir2.adapter
        );
        assert!(
            view.layers.len() >= 2,
            "expected multi-wave work order, layers={:?}",
            view.layers
        );
    }

    #[test]
    fn parse_llm_drops_meta_titles() {
        let cfg = Config::default();
        let src = PathBuf::from("/tmp/x.md");
        let raw = r#"{
  "schema": "cco-plan/v1",
  "name": "x",
  "max_parallel": 2,
  "tasks": [
    {"id": "t1", "title": "id | provider | role | status", "depends_on": [], "prompt": "nope"},
    {"id": "t2", "title": "Board", "depends_on": [], "prompt": "nope"},
    {"id": "t3", "title": "实现 handoff 归并", "depends_on": [], "prompt": "do it\nCCO_DONE ok"},
    {"id": "t4", "title": "检验与收尾", "depends_on": ["t3"], "prompt": "check\nCCO_DONE ok"}
  ]
}"#;
        let ir = parse_llm_plan_output(raw, &src, &cfg).unwrap();
        assert_eq!(ir.tasks.len(), 2);
        assert_eq!(ir.tasks[0].id, "t3");
        assert_eq!(ir.tasks[1].id, "t4");
    }
}
