//! Project light memory use cases (P2-2 · last_summary + pin) + P3 semantic memory pilot.
//!
//! [INPUT]: Config · project path · run_id · pin key/value · split outcome
//! [OUTPUT]: ProjectMemoryView · set summary from run · pin CRUD · semantic split context
//! [POS]: Application 层；Presentation 经 Tauri/CLI 调本模块
//! [PROTOCOL]: 变更时更新此头部与 src/app/CLAUDE.md
//!
//! Hard rules:
//! - Memory failures are best-effort (finish round still succeeds).
//! - Pins inject chat/planner prompts **as context only** — no route rewrite, no auto-confirm.
//! - No Dream / timeline / cross-project persona.
//! - P3 semantic memory: gated by `config.memory.enabled` (default off · zero behavior change);
//!   retrieval injects **context only** — never rewrites route / confirm / task graph.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::report::{plan_short_name, report_summary_line};
use crate::runtime::provider::TaskStatus;
use crate::state::project_memory::{
    self, compose_last_summary, ProjectLastSummary, ProjectMemoryView, ProjectPin,
};
use crate::state::{self, RunState, RunStatus};

/// Load memory for a project (empty view if none).
pub fn get(config: &Config, project: &Path) -> Result<ProjectMemoryView> {
    let pid = project_id(project);
    project_memory::get_memory(config, &pid)
}

/// Get last summary only.
pub fn last_summary(config: &Config, project: &Path) -> Result<Option<ProjectLastSummary>> {
    let pid = project_id(project);
    project_memory::get_last_summary(config, &pid)
}

/// List pins (≤3).
pub fn list_pins(config: &Config, project: &Path) -> Result<Vec<ProjectPin>> {
    let pid = project_id(project);
    project_memory::list_pins(config, &pid)
}

/// Upsert pin (hard cap 3).
pub fn upsert_pin(config: &Config, project: &Path, key: &str, value: &str) -> Result<ProjectPin> {
    let pid = project_id(project);
    project_memory::upsert_pin(config, &pid, key, value)
}

/// Delete pin by key.
pub fn delete_pin(config: &Config, project: &Path, key: &str) -> Result<bool> {
    let pid = project_id(project);
    project_memory::delete_pin(config, &pid, key)
}

/// Write last_summary from a finished run using the **rule template** (no LLM).
/// Best-effort: returns Ok even if sqlite fails after logging (via try_*).
pub fn writeback_from_run(
    config: &Config,
    run_id: &str,
    residual_note: Option<&str>,
) -> Result<Option<ProjectLastSummary>> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let rs = RunState::load(&dir)?;
    let text = rule_summary_from_run(&rs, residual_note);
    let pid = project_id(&rs.project_root);
    match project_memory::set_last_summary(config, &pid, &text) {
        Ok(row) => Ok(Some(row)),
        Err(e) => {
            tracing::warn!(error = %e, run_id = %run_id, "writeback last_summary failed");
            Ok(None)
        }
    }
}

/// Best-effort writeback (never fails the accept/finish path).
pub fn try_writeback_from_run(config: &Config, run_id: &str, residual_note: Option<&str>) {
    if let Err(e) = writeback_from_run(config, run_id, residual_note) {
        tracing::warn!(error = %e, run_id = %run_id, "try_writeback_from_run failed");
    }
}

/// Prompt context block for chat/planner (empty when none).
pub fn prompt_context(config: &Config, project: &Path) -> String {
    let pid = project_id(project);
    project_memory::try_format_memory_context(config, &pid)
}

fn project_id(project: &Path) -> String {
    project.to_string_lossy().trim_end_matches('/').to_string()
}

// ── P3 semantic memory pilot (agentmemory-integration-plan-2026-08-12) ──

/// Store config under `<state_root>/memory`; None when memory is disabled.
fn semantic_store_cfg(config: &Config) -> Option<crate::state::memory_store::MemoryConfig> {
    if !config.memory.enabled {
        return None;
    }
    Some(crate::state::memory_store::MemoryConfig {
        storage_root: config.state_root.join("memory"),
        ttl_days: config.memory.ttl_days,
        max_entries: config.memory.max_entries,
        model_path: None, // auto-detect ~/.cco/models (stub embedding when absent)
    })
}

/// Open the local semantic store under `<state_root>/memory` with config limits.
/// Returns None when disabled or the store cannot be opened (best-effort).
fn open_semantic_store(config: &Config) -> Option<crate::state::memory_store::MemoryStore> {
    let store_cfg = semantic_store_cfg(config)?;
    match crate::state::memory_store::MemoryStore::new(store_cfg) {
        Ok(store) => Some(store),
        Err(e) => {
            tracing::warn!(error = %e, "semantic memory store open failed (skipping)");
            None
        }
    }
}

/// Async `MemoryPort` handle for the scheduler (P3 场景 2 · cost router history).
/// None when memory is disabled — callers must treat that as "no memory".
pub fn semantic_port(config: &Config) -> Option<std::sync::Arc<dyn crate::ports::MemoryPort>> {
    let cfg = semantic_store_cfg(config)?;
    Some(std::sync::Arc::new(
        crate::state::memory_store::LocalMemory::new(cfg),
    ))
}

/// Retrieve past split cases relevant to `query` as a prompt context block.
/// Empty string when memory is disabled, store unavailable, or no hits.
/// Context only — the planner prompt rules still fully govern the output plan.
pub fn semantic_split_context(config: &Config, project: &Path, query: &str, limit: usize) -> String {
    let Some(mut store) = open_semantic_store(config) else {
        return String::new();
    };
    let hits = match store.search(query, limit) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "semantic memory search failed (skipping)");
            return String::new();
        }
    };
    // Prefer same-project memories; keep cross-project ones as secondary reference.
    let pid = project_id(project);
    let mut lines = Vec::new();
    for hit in &hits {
        let same_project = hit.metadata.project_id.as_deref() == Some(pid.as_str());
        let origin = if same_project { "本项目" } else { "其他项目" };
        lines.push(format!("- [{origin}] {}", hit.content.replace('\n', " ")));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "\n## 历史拆分记忆（仅参考，不构成硬约束）\n{}\n",
        lines.join("\n")
    )
}

/// Store a successful Mode B split as a memory entry (best-effort, never fails caller).
pub fn remember_split_success(
    config: &Config,
    project: &Path,
    job_id: &str,
    plan_name: &str,
    tasks: &[crate::plan::TaskIR],
) {
    let Some(mut store) = open_semantic_store(config) else {
        return;
    };
    let titles: Vec<String> = tasks.iter().map(|t| format!("「{}」", t.title)).collect();
    // Leading tokens ("拆分" · project · plan name) are space-separated so the
    // BM25 tokenizer aligns them with the retrieval query in semantic_split_context.
    let project_name = project.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let content = format!(
        "拆分 {project_name} {plan_name} 成功：{} 个任务：{}",
        tasks.len(),
        titles.join(" · ")
    );
    let metadata = crate::ports::memory::Metadata {
        project_id: Some(project_id(project)),
        task_role: None,
        provider: None,
        outcome: Some("success".into()),
        tags: vec!["split".into(), "planner".into()],
        ..Default::default()
    };
    let key = format!("split-{job_id}");
    if let Err(e) = store.store(&key, &content, metadata) {
        tracing::warn!(error = %e, key = %key, "remember_split_success failed (skipping)");
    }
}

fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Init => "初始化",
        RunStatus::Validated => "已校验",
        RunStatus::Running => "进行中",
        RunStatus::Paused => "已暂停",
        RunStatus::Completed => "已完成",
        RunStatus::Failed => "失败",
        RunStatus::Aborted => "已中止",
    }
}

fn rule_summary_from_run(rs: &RunState, residual_note: Option<&str>) -> String {
    let stem = plan_short_name(&rs.plan_path);
    let total = rs.tasks.len();
    let done = rs
        .tasks
        .values()
        .filter(|t| t.status == TaskStatus::Done)
        .count();
    // Prefer compact rule template; report_summary_line kept available for richer UI later.
    let _line = report_summary_line(rs);
    compose_last_summary(&stem, status_label(rs.status), done, total, residual_note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::sqlite::reset_for_test;
    use crate::state::{RunState, RunStatus, TaskState};
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn writeback_from_run_persists_summary() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        let runs = cfg.runs_dir();
        std::fs::create_dir_all(&runs).unwrap();
        let run_id = "20260722T000000Z-mem1";
        let run_dir = runs.join(run_id);
        std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let mut tasks = HashMap::new();
        let mut t1 = TaskState::pending("claude", "print");
        t1.status = TaskStatus::Done;
        tasks.insert("t1".into(), t1);
        let mut t2 = TaskState::pending("claude", "print");
        t2.status = TaskStatus::Done;
        tasks.insert("t2".into(), t2);
        let rs = RunState {
            schema: "cco-run/v1".into(),
            run_id: run_id.into(),
            project_root: project.clone(),
            plan_path: project.join("docs/demo.md"),
            adapter: "cco-plan/v1".into(),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            status: RunStatus::Completed,
            tasks,
            auto_commits: vec![],
            run_dir: run_dir.clone(),
        };
        rs.save().unwrap();

        let row = writeback_from_run(&cfg, run_id, None)
            .unwrap()
            .expect("summary written");
        assert!(row.text.contains("demo"));
        assert!(row.text.contains("2/2") || row.text.contains("完成 2"));
        let mem = get(&cfg, &project).unwrap();
        assert!(mem.last_summary.is_some());
    }

    fn mk_task(id: &str, title: &str) -> crate::plan::TaskIR {
        crate::plan::TaskIR {
            id: id.into(),
            title: title.into(),
            depends_on: vec![],
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: "p\nCCO_DONE ok".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec![],
            wait_for: vec![],
        }
    }

    #[test]
    fn semantic_context_empty_when_disabled() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        assert!(!cfg.memory.enabled, "memory must default off");
        let ctx = semantic_split_context(&cfg, Path::new("/tmp/p"), "拆分 demo", 3);
        assert!(ctx.is_empty(), "disabled memory must not inject context");
    }

    #[test]
    fn semantic_split_memory_roundtrip() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        cfg.memory.enabled = true;
        let project = dir.path().join("tauri-app");

        let tasks = vec![mk_task("t1", "实现登录页"), mk_task("t2", "检验员终检（可选）")];
        remember_split_success(&cfg, &project, "job-1", "tauri-app-plan", &tasks);

        let ctx = semantic_split_context(&cfg, &project, "拆分 tauri-app greenfield", 3);
        assert!(ctx.contains("历史拆分记忆"), "ctx={ctx}");
        assert!(ctx.contains("实现登录页"), "ctx={ctx}");
        assert!(ctx.contains("[本项目]"), "ctx={ctx}");
    }

    #[test]
    fn pin_crud_via_app() {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        let project = Path::new("/tmp/app-mem-pins");
        upsert_pin(&cfg, project, "stack", "rust").unwrap();
        upsert_pin(&cfg, project, "ui", "tauri").unwrap();
        let pins = list_pins(&cfg, project).unwrap();
        assert_eq!(pins.len(), 2);
        assert!(delete_pin(&cfg, project, "ui").unwrap());
        assert_eq!(list_pins(&cfg, project).unwrap().len(), 1);
    }
}
