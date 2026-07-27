//! Dedicated split agent (ModelSplitAgent) — cco-split/v1 producer.
//!
//! [INPUT]: plan markdown · Config
//! [OUTPUT]: CcoSplitJob · PlanIR snapshot helpers
//! [POS]: plan — OpenHands Plan Mode 气质；执行仍走 Worker
//! [PROTOCOL]: 变更时更新此头部与 src/plan/CLAUDE.md · docs/openhands-style-split-agent-landing-2026-07-21.md

mod extract;
mod model;
mod parse;
mod prompt;
mod repo_digest;

pub use extract::extract_json_object;
pub use model::{FixtureSplitAgent, ModelSplitAgent};
pub use parse::parse_agent_output;
pub use prompt::{system_prompt, user_prompt};

use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::Config;
use crate::domain::plan::{to_plan_ir, CcoSplitJob, PlanIR};
use crate::ports::split_agent::{SplitAgentPort, SplitRequest};

use super::planner::{append_log, job_dir, PlanJob};

/// Run ModelSplitAgent (or env fixture) → soft-accepted CcoSplit → optional SoT save → PlanIR.
///
/// Adapter tag: `split-agent-llm` (also recognizable as llm path).
pub fn build_split_agent_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let source_text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    let max_parallel = job
        .max_parallel
        .unwrap_or(config.default.max_parallel)
        .clamp(1, 32);
    let now = Utc::now().to_rfc3339();
    let req = SplitRequest {
        job_id: job.job_id.clone(),
        project: job.project.clone(),
        plan_path: job.plan_path.clone(),
        plan_abs: abs.clone(),
        plan_md: source_text,
        max_parallel,
        created_at: job.created_at.to_rfc3339(),
        updated_at: now.clone(),
        grain_hint: job.grain_hint.clone(),
        revision_notes: job.revision_notes.clone(),
        effort: job.effort.clone(),
    };

    if let Some(ref g) = job.grain_hint {
        if !g.trim().is_empty() {
            append_log(
                config,
                &job.job_id,
                &format!("ModelSplitAgent grain: {}", g.trim()),
            );
        }
    }
    if let Some(ref n) = job.revision_notes {
        if !n.trim().is_empty() {
            let preview: String = n.trim().chars().take(80).collect();
            append_log(
                config,
                &job.job_id,
                &format!("ModelSplitAgent revision_notes: {preview}"),
            );
        }
    }
    {
        let hints = repo_digest::extract_path_hints(
            &std::fs::read_to_string(&abs).unwrap_or_default(),
        );
        if !hints.is_empty() {
            append_log(
                config,
                &job.job_id,
                &format!("ModelSplitAgent path_hints: {}", hints.join(", ")),
            );
        } else {
            append_log(config, &job.job_id, "ModelSplitAgent repo_digest: shallow top-level only");
        }
    }
    if let Some(ref e) = job.effort {
        append_log(
            config,
            &job.job_id,
            &format!("ModelSplitAgent effort: {e}"),
        );
    }
    append_log(
        config,
        &job.job_id,
        "ModelSplitAgent: splitting plan → cco-split/v1…",
    );

    let agent = ModelSplitAgent::new(config);
    let mut doc = agent.split(&req)?;
    doc.updated_at = Utc::now().to_rfc3339();
    let notes = crate::domain::plan::soft_accept_split(&mut doc);
    for n in &notes {
        append_log(config, &job.job_id, &format!("split_agent soft_accept: {n}"));
    }

    // Persist SoT early so desk can load even if later write_proposed races.
    crate::state::cco_split_store::try_save_cco_split(config, &doc);
    append_log(
        config,
        &job.job_id,
        &format!(
            "ModelSplitAgent ok: {} tasks, max_parallel={}, source={}",
            doc.tasks.len(),
            doc.max_parallel,
            doc.source.as_str()
        ),
    );

    // Debug raw snapshot path for support (best-effort).
    let _ = std::fs::write(
        job_dir(config, &job.job_id).join("cco_split_agent.json"),
        serde_json::to_string_pretty(&doc).unwrap_or_default(),
    );

    cco_split_to_plan_ir(&doc, job)
}

/// Materialize PlanIR for finish_plan_job / write_proposed dual snapshot.
pub fn cco_split_to_plan_ir(doc: &CcoSplitJob, job: &PlanJob) -> Result<PlanIR> {
    let mut ir = to_plan_ir(doc, &job.provider, &job.exec_mode);
    ir.adapter = "split-agent-llm".into();
    // Keep cco-split/llm prefix so write_proposed source tagging stays Llm.
    if !ir.adapter.starts_with("cco-split/") {
        ir.adapter = format!("cco-split/llm+{}", ir.adapter);
    }
    crate::domain::worker::apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
    crate::plan::apply_tag_routing(&mut ir);
    crate::plan::materialize_role_defaults(&mut ir);
    // Soft collab fixes if any advanced fields leaked.
    let soft = crate::domain::plan::soften_plan_for_accept(&mut ir);
    for n in soft {
        let _ = n;
    }
    ir.validate()?;
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::plan::planner::{get_plan_job, start_plan_job, StartPlanJobRequest};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// Serialize env mutation so parallel tests do not inherit fixture JSON.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// P2-5: fixture JSON → plan_mode=ai → SQLite cco_split has tasks.
    #[test]
    fn start_plan_job_ai_uses_split_agent_fixture() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("idea.md");
        // Prose plan (raw-single) so we don't short-circuit on structured parse.
        std::fs::write(
            &plan,
            "做一个小发布：先写程序入口，再补单测，文档可选。\n",
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let fixture = r#"{"schema":"cco-split/v1","title":"发布","max_parallel":2,"tasks":[
          {"id":"t1","title":"写入口","body":"实现 main 入口","depends_on":[],"kind":"do","optional":false,"enabled":true},
          {"id":"t2","title":"补单测","body":"覆盖入口","depends_on":["t1"],"kind":"check","optional":false,"enabled":true},
          {"id":"t3","title":"可选文档","body":"写 README","depends_on":["t1"],"kind":"do","optional":true,"enabled":false}
        ]}"#;
        std::env::set_var("CCO_SPLIT_AGENT_JSON", fixture);
        let _guard = EnvClearGuard;

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("idea.md"),
                plan_mode: Some("ai".into()),
                // Not fake — fake forces heuristic and skips agent.
                provider: Some("claude".into()),
                mode: Some("print".into()),
                max_parallel: Some(2),
                preserve_from_job_id: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
            },
        )
        .unwrap();
        let mut view = view;
        for _ in 0..120 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            view = get_plan_job(&cfg, &view.job_id).unwrap();
        }
        std::env::remove_var("CCO_SPLIT_AGENT_JSON");
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        assert!(
            view.adapter.as_deref().unwrap_or("").contains("llm")
                || view.adapter.as_deref().unwrap_or("").contains("split-agent"),
            "adapter={:?}",
            view.adapter
        );
        let sot = crate::state::cco_split_store::load_cco_split(&cfg, &view.job_id)
            .unwrap()
            .expect("cco_split SoT row");
        assert_eq!(sot.tasks.len(), 3);
        assert_eq!(sot.tasks[0].task_id, "t1");
        assert!(sot.tasks[2].optional);
        assert!(!sot.tasks[2].enabled);
        assert!(view.task_count.unwrap_or(0) >= 3);
    }

    /// P5-3 non-dev script (lib): fast local split → desk fields → confirm open-run.
    #[test]
    fn fast_path_desk_fields_and_confirm() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("ship.md");
        std::fs::write(
            &plan,
            "## 做入口\n\n写程序入口\n\n## 补测\n\n加单测\n\n## 可选文档\n\n写 README\n",
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(cfg.runs_dir()).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("ship.md"),
                plan_mode: Some("fast".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(2),
                preserve_from_job_id: None,
            grain_hint: None,
            revision_notes: None,
            effort: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        assert!(view.task_count.unwrap_or(0) >= 2);
        // Desk must show order / wave / include for non-dev checklist.
        assert!(view.tasks.iter().any(|t| t.wave.is_some()));
        assert!(view.tasks.iter().any(|t| t.ord.is_some()));
        let sot = crate::state::cco_split_store::load_cco_split(&cfg, &view.job_id)
            .unwrap()
            .expect("SoT after fast split");
        assert!(!sot.tasks.is_empty());
        assert!(crate::plan::run_gate_ok(&sot).is_ok());

        let run_id = crate::app::split::confirm(cfg.clone(), &view.job_id, None).unwrap();
        assert!(!run_id.is_empty());
        let job = crate::plan::planner::PlanJob::load(&cfg, &view.job_id).unwrap();
        assert_eq!(
            job.status,
            crate::plan::planner::PlanJobStatus::Confirmed
        );
        let sot2 = crate::state::cco_split_store::load_cco_split(&cfg, &view.job_id)
            .unwrap()
            .expect("SoT after confirm");
        assert_eq!(sot2.status, crate::plan::CcoSplitStatus::Confirmed);
        assert_eq!(sot2.run_id.as_deref(), Some(run_id.as_str()));
    }

    /// Chat plan-card「直接执行」: whole md → single task → confirm open-run (no multi-split).
    #[test]
    fn direct_path_single_task_and_confirm() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let plan = project.join("fix.md");
        std::fs::write(
            &plan,
            "# 云山藏 · 展示站修复\n\n## 做\n\n- 修底 CTA\n- 修 Footer 色阶\n",
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(cfg.runs_dir()).unwrap();

        let view = start_plan_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("fix.md"),
                plan_mode: Some("direct".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(4),
                preserve_from_job_id: None,
                grain_hint: None,
                revision_notes: None,
                effort: None,
            },
        )
        .unwrap();
        assert_eq!(view.status, "planned", "err={:?}", view.error);
        assert_eq!(view.plan_mode, "direct");
        // Whole document is one business task (system post may append optionals).
        let business: Vec<_> = view
            .tasks
            .iter()
            .filter(|t| !crate::domain::plan::is_system_post_task(&t.id))
            .collect();
        assert_eq!(business.len(), 1, "business tasks={:?}", business);
        assert!(
            business[0].title.contains("云山藏") || business[0].title.contains("展示站"),
            "title={}",
            business[0].title
        );
        let ir = crate::plan::planner::load_proposed(&cfg, &view.job_id).unwrap();
        // SoT round-trip tags adapter as cco-split/{source}; still one business task.
        assert_eq!(ir.max_parallel, 1);
        assert_eq!(
            ir.tasks
                .iter()
                .filter(|t| !crate::domain::plan::is_system_post_task(&t.id))
                .count(),
            1
        );

        let run_id = crate::app::split::confirm(cfg.clone(), &view.job_id, None).unwrap();
        assert!(!run_id.is_empty());
    }

    /// Clear fixture env even if test panics mid-way.
    struct EnvClearGuard;
    impl Drop for EnvClearGuard {
        fn drop(&mut self) {
            std::env::remove_var("CCO_SPLIT_AGENT_JSON");
        }
    }
}
