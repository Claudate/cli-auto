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
//! Limits: `PLANNER_MAX_TASKS`（拆解软上限）· `MAX_TASKS`（含系统收尾后硬上限）；
//! stream-json 从最终 `type=result` 取 plan，勿取 init 事件的首个 `{`。

mod digest;
mod heuristic;
mod job;
mod llm;
mod task_edit;
mod view;

pub use job::{
    get_plan_job, job_dir, latest_plan_job_for_project, plan_jobs_dir, start_plan_job, PlanJob,
    PlanJobStatus, StartPlanJobRequest,
};
pub use view::{
    apply_user_edits_to_ir, job_view, load_proposed, load_proposed_for_exec, load_user_edits,
    mark_confirmed, planner_cost_for_run, remove_proposed_task, sanitize_proposed_deps,
    update_proposed_task, write_user_edits, PlanJobView, PlanTaskView, PlanUserEdits,
    SanitizeDepsResult, TaskUserEdit,
};

#[cfg(test)]
mod tests {
    use super::heuristic::merge_sections;
    use super::llm::{extract_json_object, parse_llm_plan_output, LlmPlanDoc};
    use super::*;
    use crate::config::Config;
    use crate::plan::{MAX_TASKS, PLANNER_MAX_TASKS};
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        assert!(view.task_count.unwrap() >= 3);
        assert!(!view.layers.is_empty());
        assert_eq!(view.layers[0].len(), 2); // t1,t2 parallel
        // 默认系统收尾关：不应出现 sys-post-*
        assert!(
            !view
                .tasks
                .iter()
                .any(|t| t.id.starts_with("sys-post-")),
            "system post should be off by default"
        );
    }

    #[test]
    fn system_post_tasks_injected_when_enabled() {
        use crate::plan::{SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# hello\n\ndo something cool\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        cfg.default.post_inspect_enabled = true;
        cfg.default.post_git_push_enabled = true;
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        let ids: Vec<_> = view.tasks.iter().map(|t| t.id.as_str()).collect();
        assert!(
            ids.contains(&SYS_POST_INSPECT_ID),
            "missing inspect: {ids:?}"
        );
        assert!(
            ids.contains(&SYS_POST_GIT_PUSH_ID),
            "missing push: {ids:?}"
        );
        let inspect = view
            .tasks
            .iter()
            .find(|t| t.id == SYS_POST_INSPECT_ID)
            .unwrap();
        let push = view
            .tasks
            .iter()
            .find(|t| t.id == SYS_POST_GIT_PUSH_ID)
            .unwrap();
        assert!(inspect.optional && inspect.include, "inspect default checked");
        assert!(push.optional && push.include, "push default checked");
        assert!(
            push.depends_on.iter().any(|d| d == SYS_POST_INSPECT_ID),
            "push should depend on inspect"
        );
        // last layer should include system posts
        let last = view.layers.last().expect("layers");
        assert!(
            last.iter().any(|id| id == SYS_POST_GIT_PUSH_ID)
                || last.iter().any(|id| id == SYS_POST_INSPECT_ID),
            "system posts should appear in a late wave: {last:?}"
        );
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
                preserve_from_job_id: None,
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
                preserve_from_job_id: None,
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
    fn finish_plan_job_persists_critic_notes_for_landed_doc() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("landed.md");
        std::fs::write(
            &plan,
            r#"# landed plan

> 状态：**已落地**（H0–H4 全 PASS）

## 目标
回归验证既有能力。
"#,
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("landed.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: None,
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");
        assert_eq!(view.digest_mode.as_deref(), Some("regression"));
        assert!(
            view.critic_summary.is_some(),
            "critic_summary should be set"
        );
        // fake demo graph has no role=inspect task → regression note expected
        assert!(
            view.critic_notes.iter().any(|n| n.contains("检验")),
            "expected missing-inspect note, got {:?}",
            view.critic_notes
        );
        assert!(view.critic_edges_removed.is_some());
        assert!(view.critic_titles_rewritten.is_some());
        assert!(view.critic_prompts_tagged.is_some());
        // fake provider never runs LLM second pass
        assert_eq!(view.critic_llm_used, Some(false));
    }

    #[test]
    fn sanitize_proposed_deps_drops_unmotivated_edges() {
        use super::view::{load_proposed, sanitize_proposed_deps, write_proposed};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned");

        // Inject an unmotivated edge t2 → t1 into proposed plan.
        let mut ir = load_proposed(&cfg, &view.job_id).unwrap();
        assert!(ir.tasks.len() >= 2, "fake plan should have ≥2 tasks");
        let t1 = ir.tasks[0].id.clone();
        ir.tasks[1].depends_on = vec![t1.clone()];
        // Ensure prompt does NOT mention t1 so sanitize drops it.
        ir.tasks[1].prompt = "do orthogonal work only\nCCO_DONE ok".into();
        write_proposed(&cfg, &view.job_id, &ir).unwrap();

        let res = sanitize_proposed_deps(&cfg, &view.job_id).unwrap();
        assert!(res.removed >= 1, "should drop unmotivated edge");
        let ir2 = load_proposed(&cfg, &view.job_id).unwrap();
        assert!(
            ir2.tasks[1].depends_on.is_empty(),
            "edge should be gone: {:?}",
            ir2.tasks[1].depends_on
        );
        assert_eq!(res.view.status, "planned");
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
                preserve_from_job_id: None,
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
    fn latest_job_skips_stale_planning_zombie() {
        use super::job::{job_dir, PlanJob, PlanJobStatus};
        use chrono::{Duration, Utc};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\n## a\n\ndo a\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let planned = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(2),
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        assert_eq!(planned.status, "planned");

        // 更晚但已超时的 zombie planning 不得盖住 planned
        let zombie_id = "plan-zombie-planning-test";
        let zombie_dir = job_dir(&cfg, zombie_id);
        std::fs::create_dir_all(&zombie_dir).unwrap();
        let zombie = PlanJob {
            job_id: zombie_id.into(),
            status: PlanJobStatus::Planning,
            project: project.clone(),
            plan_path: PathBuf::from("idea.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now() - Duration::hours(2),
            updated_at: Utc::now() - Duration::minutes(45),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
        };
        zombie.save(&cfg).unwrap();

        let latest = latest_plan_job_for_project(&cfg, &project)
            .unwrap()
            .expect("should find latest");
        assert_eq!(latest.job_id, planned.job_id);
        assert_eq!(latest.status, "planned");
    }

    #[test]
    fn get_plan_job_reaps_stale_planning_zombie() {
        use super::job::{get_plan_job, job_dir, PlanJob, PlanJobStatus};
        use chrono::{Duration, Utc};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let zombie_id = "plan-reap-zombie-test";
        let zombie_dir = job_dir(&cfg, zombie_id);
        std::fs::create_dir_all(zombie_dir.join("llm_work/tasks/__planner__")).unwrap();
        // Dead pid (unlikely to be alive)
        // High unused pid — process_alive should be false
        std::fs::write(
            zombie_dir.join("llm_work/tasks/__planner__/meta.json"),
            r#"{"pid": 999999, "opaque_id": "pid:999999"}"#,
        )
        .unwrap();
        std::fs::write(zombie_dir.join("planner.log"), "started\n").unwrap();
        // Touch log mtime old? reap uses age_created > 45s with dead pid
        let zombie = PlanJob {
            job_id: zombie_id.into(),
            status: PlanJobStatus::Planning,
            project: project.clone(),
            plan_path: PathBuf::from("idea.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now() - Duration::minutes(5),
            updated_at: Utc::now() - Duration::minutes(5),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
        };
        zombie.save(&cfg).unwrap();

        let view = get_plan_job(&cfg, zombie_id).unwrap();
        assert_eq!(view.status, "plan_failed", "err={:?}", view.error);
        assert!(
            view.error
                .as_deref()
                .map(|e| e.contains("process gone") || e.contains("timeout") || e.contains("stale"))
                .unwrap_or(false),
            "expected reap reason, got {:?}",
            view.error
        );
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
    fn parse_llm_plan_output_provider_role_scope_tags() {
        use crate::plan::TaskRole;
        let dir = tempdir().unwrap();
        let src = dir.path().join("plan.md");
        std::fs::write(&src, "# multi\n").unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        cfg.default.default_provider = "claude".into();

        let plan_body = r#"{
          "schema":"cco-plan/v1",
          "name":"mixed",
          "max_parallel":2,
          "tasks":[
            {
              "id":"a",
              "title":"实现 A",
              "depends_on":[],
              "provider":"codex",
              "role":"implement",
              "tags":["codex","backend"],
              "scope":{"paths":["src/a/**"],"readonly":[],"forbid":[]},
              "outputs":[".cco-out/a/SUMMARY.md"],
              "prompt":"做 A\nCCO_DONE ok"
            },
            {
              "id":"insp",
              "title":"检验员终检",
              "depends_on":["a"],
              "tags":["inspect"],
              "prompt":"对照计划验收\nCCO_DONE ok"
            }
          ]
        }"#;
        let ir = parse_llm_plan_output(plan_body, &src, &cfg).unwrap();
        assert_eq!(ir.tasks.len(), 2);
        let a = ir.tasks.iter().find(|t| t.id == "a").unwrap();
        assert_eq!(a.provider, "codex");
        assert_eq!(a.role, Some(TaskRole::Implement));
        assert!(a.tags.iter().any(|t| t == "codex"));
        assert_eq!(
            a.scope.as_ref().map(|s| s.paths.as_slice()),
            Some(["src/a/**".to_string()].as_slice())
        );
        assert!(a.outputs.iter().any(|o| o.contains("SUMMARY")));
        let insp = ir.tasks.iter().find(|t| t.id == "insp").unwrap();
        // Title+tag inferred inspect role
        assert_eq!(insp.role, Some(TaskRole::Inspect));
        assert!(insp.tags.iter().any(|t| t == "inspect"));
    }

    #[test]
    fn merge_sections_caps_at_max() {
        let sections: Vec<_> = (0..35)
            .map(|i| (format!("s{i}"), format!("body {i}")))
            .collect();
        let merged = merge_sections(sections, PLANNER_MAX_TASKS);
        assert_eq!(merged.len(), PLANNER_MAX_TASKS);
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
                preserve_from_job_id: None,
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
        // No W0/W1 windows → still falls back to meta work-order template.
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
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
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

        // Spec with implement ## titles but no W0/A1 — must RECOVER, not silent meta.
        let recover_plan = project.join("recover-headings.md");
        let recover_md = r#"# 某功能落地

> 关联真源 · PROTOCOL · 非目标 · 成功标准

## 0. 一句话

做入口路由。

## 8. 非目标

不做 IDE。

## 实现 resolveEntryRoute

改 sessionEntry.js，planned 落拆分台。

完成定义：杀进程重开仍见拆分台。

## 实现 chatAssignDirect 默认

改 state.js 默认直拆。

文件：web/js/state.js

自测：清 localStorage 后拆。

## 10. 成功标准

S1

## 12. 修订历史

t1
"#;
        std::fs::write(&recover_plan, recover_md).unwrap();
        assert!(looks_like_spec_document(recover_md));
        let job_r = PlanJob {
            job_id: "plan-test-recover".into(),
            plan_path: PathBuf::from("recover-headings.md"),
            ..job.clone()
        };
        std::fs::create_dir_all(job_dir(&cfg, &job_r.job_id)).unwrap();
        let ir_r = build_heuristic_ai_plan(&cfg, &job_r).expect("recover heuristic");
        let titles_r: Vec<_> = ir_r.tasks.iter().map(|t| t.title.clone()).collect();
        assert!(
            titles_r.iter().any(|t| t.contains("resolveEntryRoute") || t.contains("实现")),
            "must recover implement headings, got {titles_r:?}"
        );
        assert!(
            titles_r.iter().all(|t| !t.contains("读懂目标与范围")),
            "must not abandon to meta template when headings recoverable, got {titles_r:?}"
        );

        // 派工/落地计划 #### A1 · … must become those tasks — NOT the meta 4-wave.
        let dispatch = project.join("ux-nondev-landing.md");
        let dispatch_md = r#"# cco 非开发主路径 · 落地实施计划

> 角色：体验落地实施真源 · 不排期则不碰 · PROTOCOL

## 0. 目标 / 非目标

给 PM 用。

## 3. 任务表

### 波次 A — 入口与减法（P0 · MVP Ship）

#### A1 · 待确认强制进拆分台

| 项 | 内容 |
|----|------|
| **文件** | sessionEntry.js |
| **改法** | resolveEntryRoute planned → workspace |

#### A2 · 主路径默认跳过「执行选项」层

state.js chatAssignDirect 默认开。

#### A3 · 拆分台顶栏只留主路径控件

index.html 藏 sanitize / writeback。

### 波次 B — 写计划顺滑

#### B1 · 聊天空态引导

空态三句示例。

#### B2 · 主 CTA：保存与拆分意图合并

静默 save 再 assign。

## 9. 与现有文档关系

参考。

## 10. 修订记录

t1
"#;
        std::fs::write(&dispatch, dispatch_md).unwrap();
        assert!(
            looks_like_spec_document(dispatch_md),
            "dispatch plan scores as spec MD (chrome keywords)"
        );
        let job_dispatch = PlanJob {
            job_id: "plan-test-dispatch".into(),
            plan_path: PathBuf::from("ux-nondev-landing.md"),
            ..job.clone()
        };
        std::fs::create_dir_all(job_dir(&cfg, &job_dispatch.job_id)).unwrap();
        let ir_d = build_heuristic_ai_plan(&cfg, &job_dispatch).expect("dispatch heuristic");
        let titles_d: Vec<_> = ir_d.tasks.iter().map(|t| t.title.clone()).collect();
        assert!(
            titles_d.iter().any(|t| t.contains("A1") || t.contains("待确认")),
            "dispatch plan must split into #### A1… tasks, got {titles_d:?}"
        );
        assert!(
            titles_d.iter().any(|t| t.contains("A2") || t.contains("执行选项")),
            "expected A2 task, got {titles_d:?}"
        );
        assert!(
            titles_d.iter().all(|t| {
                !t.contains("读懂目标与范围")
                    && !t.contains("拆出可执行工作包")
                    && !t.contains("专门巡检")
            }),
            "must NOT use meta work-order titles for #### task plans, got {titles_d:?}"
        );
        assert!(
            ir_d.tasks.len() >= 4,
            "expected several #### tasks, got {} {titles_d:?}",
            ir_d.tasks.len()
        );

        // Landing plan with W0/W1… must become those phases — NOT the meta 4-wave.
        let landing = project.join("StoryForge-landing.md");
        let landing_md = r#"# StoryForge 落地计划

> 关联真源 · PROTOCOL · GEB 入口 · 不排期则不碰

## 0. 一句话

焊章生命周期四块。

## 1. 为什么做（问题陈述）

痛点表。

## 8. 非目标

N1

## 5. 分期与窗（W 纵队）

### W0 · 规格冻结与测量（0.5–1 天）

- [ ] 本文件评审通过
- [ ] 选定 1 本长测书

**完成判据**：存储路径拍板。

### W1 · Handoff + Envelope + 写注入（P0 主刀）

1. ContinuityHandoff 类型
2. Settle 生成 handoff
3. Write 前 Envelope

**Acceptance**：连续写 2 章。

### W2 · ChapterPlan 字段 + PlanReconciliation

Memo 扩展 goals。

### W3 · Fact Canon 门

fact status candidate→confirmed。

### W4 · 可观测 + CanonCheck（P1）

injection 分层可见。

### W5 · 细纲 scenes（P2 · 可选）

scenes 落盘。

## 10. 成功标准

S1

## 12. 修订历史

t1
"#;
        std::fs::write(&landing, landing_md).unwrap();
        assert!(
            looks_like_spec_document(landing_md),
            "landing plan is still a spec MD"
        );
        let job2 = PlanJob {
            job_id: "plan-test-landing".into(),
            plan_path: PathBuf::from("StoryForge-landing.md"),
            ..job.clone()
        };
        std::fs::create_dir_all(job_dir(&cfg, &job2.job_id)).unwrap();
        let ir2 = build_heuristic_ai_plan(&cfg, &job2).expect("landing heuristic");
        let titles2: Vec<_> = ir2.tasks.iter().map(|t| t.title.clone()).collect();
        assert!(
            titles2.iter().any(|t| t.contains("W0") || t.contains("W1")),
            "landing plan must split into W-windows, got {titles2:?}"
        );
        assert!(
            titles2.iter().all(|t| {
                !t.contains("读懂目标与范围")
                    && !t.contains("拆出可执行工作包")
                    && !t.contains("专门巡检")
            }),
            "must NOT use meta work-order titles, got {titles2:?}"
        );
        assert!(
            !ir2.require_inspect,
            "W-window split should not force meta inspect tail; got require_inspect=true titles={titles2:?}"
        );
        assert!(
            ir2.tasks.iter().all(|t| t.role != Some(crate::plan::TaskRole::Inspect)),
            "no forced inspect role on landing phases: {titles2:?}"
        );
        // Sequential deps: t2 waits t1
        if ir2.tasks.len() >= 2 {
            assert!(
                ir2.tasks[1].depends_on.contains(&"t1".into()),
                "phases should be sequential, deps={:?}",
                ir2.tasks[1].depends_on
            );
        }

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
                preserve_from_job_id: None,
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

    /// P2-1: delete a task and rewrite depends_on; refuse empty plan.
    #[test]
    fn remove_proposed_task_rewrites_deps() {
        use super::view::{load_proposed, remove_proposed_task};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        let before = view.task_count.unwrap_or(0);
        assert!(before >= 3);
        let drop_id = view.tasks[0].id.clone();
        let view2 = remove_proposed_task(&cfg, &view.job_id, &drop_id).unwrap();
        assert_eq!(view2.task_count, Some(before - 1));
        let ir = load_proposed(&cfg, &view.job_id).unwrap();
        assert!(ir.tasks.iter().all(|t| t.id != drop_id));
        for t in &ir.tasks {
            assert!(
                !t.depends_on.iter().any(|d| d == &drop_id),
                "stale dep on removed task: {:?}",
                t.depends_on
            );
        }
    }

    /// P2-1: explicit depends_on patch + validate cycle reject.
    #[test]
    fn update_proposed_task_sets_depends_on() {
        use super::view::{load_proposed, update_proposed_task};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        let a = view.tasks[0].id.clone();
        let b = view.tasks[1].id.clone();
        // Make b depend on a.
        update_proposed_task(
            &cfg,
            &view.job_id,
            &b,
            None,
            None,
            None,
            None,
            Some(vec![a.clone()]),
            None,
            None,
        )
        .unwrap();
        let ir = load_proposed(&cfg, &view.job_id).unwrap();
        let tb = ir.tasks.iter().find(|t| t.id == b).unwrap();
        assert_eq!(tb.depends_on, vec![a.clone()]);

        // Self-dep rejected.
        let err = update_proposed_task(
            &cfg,
            &view.job_id,
            &b,
            None,
            None,
            None,
            None,
            Some(vec![b.clone()]),
            None,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("自己") || err.contains("itself") || err.contains("依赖"), "{err}");
    }

    /// S-role: confirm-screen can patch role + scope.paths; DTO exposes them.
    #[test]
    fn update_proposed_task_sets_role_and_scope() {
        use super::view::{load_proposed, update_proposed_task};
        use crate::plan::TaskRole;

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        let a = view.tasks[0].id.clone();
        let view2 = update_proposed_task(
            &cfg,
            &view.job_id,
            &a,
            None,
            None,
            None,
            None,
            None,
            Some("implement".into()),
            Some(vec!["src/web/**".into(), " src/web/** ".into()]),
        )
        .unwrap();
        let tv = view2.tasks.iter().find(|t| t.id == a).unwrap();
        assert_eq!(tv.role.as_deref(), Some("implement"));
        assert_eq!(
            tv.scope.as_ref().map(|s| s.paths.clone()),
            Some(vec!["src/web/**".into()])
        );

        let ir = load_proposed(&cfg, &view.job_id).unwrap();
        let ta = ir.tasks.iter().find(|t| t.id == a).unwrap();
        assert_eq!(ta.role, Some(TaskRole::Implement));
        assert_eq!(
            ta.scope.as_ref().map(|s| s.paths.clone()),
            Some(vec!["src/web/**".into()])
        );

        // Clear role.
        update_proposed_task(
            &cfg,
            &view.job_id,
            &a,
            None,
            None,
            None,
            None,
            None,
            Some("".into()),
            None,
        )
        .unwrap();
        let ir2 = load_proposed(&cfg, &view.job_id).unwrap();
        let ta2 = ir2.tasks.iter().find(|t| t.id == a).unwrap();
        assert_eq!(ta2.role, None);
    }

    /// P2-2: replan with preserve_from_job_id re-applies title/prompt/deps/removals.
    #[test]
    fn replan_preserves_user_edits_by_title() {
        use super::view::{load_proposed, remove_proposed_task, update_proposed_task};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        std::fs::write(&plan, "# x\n\ny\n").unwrap();
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
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        let t1 = view.tasks[0].id.clone();
        let t1_title = view.tasks[0].title.clone();
        let t2 = view.tasks[1].id.clone();
        let t2_title = view.tasks[1].title.clone();
        let t3 = view.tasks.get(2).map(|t| t.id.clone());

        // Rename t1 prompt + title; set t2 deps → t1; remove t3 if present.
        // Keep same provider (fake) so multi-provider worktree gate is not tripped.
        update_proposed_task(
            &cfg,
            &view.job_id,
            &t1,
            Some(format!("{t1_title}（人工）")),
            Some("HUMAN_PATCHED_PROMPT\nCCO_DONE ok".into()),
            None,
            Some("fake".into()),
            None,
            None,
            None,
        )
        .unwrap();
        // After rename, t1 still matched by original key; set t2 deps by ids.
        update_proposed_task(
            &cfg,
            &view.job_id,
            &t2,
            None,
            None,
            None,
            None,
            Some(vec![t1.clone()]),
            None,
            None,
        )
        .unwrap();
        if let Some(ref id) = t3 {
            let _ = remove_proposed_task(&cfg, &view.job_id, id);
        }

        // Fresh replan with preserve.
        let view2 = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("fake".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: None,
                preserve_from_job_id: Some(view.job_id.clone()),
            },
        )
        .unwrap();
        assert_eq!(view2.status, "planned", "err={:?}", view2.error);
        let ir = load_proposed(&cfg, &view2.job_id).unwrap();

        // Title/prompt/provider preserved on the task whose *planned* title matches original t1.
        let patched = ir
            .tasks
            .iter()
            .find(|t| t.title.contains("人工") || t.prompt.contains("HUMAN_PATCHED_PROMPT"))
            .expect("patched task should reappear");
        assert!(
            patched.prompt.contains("HUMAN_PATCHED_PROMPT"),
            "prompt lost: {}",
            patched.prompt
        );
        assert_eq!(patched.provider, "fake");

        // t2 should depend on the patched task if both still present.
        if let Some(tb) = ir.tasks.iter().find(|t| {
            normalize_or(&t.title) == normalize_or(&t2_title)
                || t.title == t2_title
        }) {
            assert!(
                tb.depends_on.iter().any(|d| d == &patched.id),
                "t2 deps should include patched: {:?} patched={}",
                tb.depends_on,
                patched.id
            );
        }

        // Removed title should not return.
        if let Some(ref id) = t3 {
            let _ = id;
            let removed_title = view
                .tasks
                .iter()
                .find(|t| t3.as_ref() == Some(&t.id))
                .map(|t| t.title.clone())
                .unwrap_or_default();
            assert!(
                !ir.tasks
                    .iter()
                    .any(|t| super::view::normalize_task_title_key(&t.title)
                        == super::view::normalize_task_title_key(&removed_title)),
                "removed task came back: {removed_title}"
            );
        }
    }

    fn normalize_or(s: &str) -> String {
        super::view::normalize_task_title_key(s)
    }
}
