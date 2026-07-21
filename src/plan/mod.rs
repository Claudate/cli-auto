//! Plan loading, adapters, and PlanIR facade (A1: pure model in domain::plan).
//!
//! [INPUT]: 计划文件路径 · config 默认 provider/mode
//! [OUTPUT]: PlanIR/TaskIR · load_plan · list_plans · materialize_* · inject_system_post_tasks
//! [POS]: plan 模块入口；**类型/校验/物化真源** = `crate::domain::plan`；本文件 = adapters + IO + 兼容 re-export
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

pub mod adapters;
pub mod planner;
pub mod system_post;

// Domain pure model (A1 extraction + CcoSplit SoT shape).
pub use crate::domain::plan::{
    apply_tag_routing, from_plan_ir, is_system_post_task, materialize_role_defaults,
    materialize_selected_tasks, normalize_optional_title, parse_role_input, recompute_waves,
    run_gate_ok, soft_accept_split, soften_plan_for_accept, split_topo_layers,
    title_is_meta_heading, title_looks_optional, to_plan_ir, CcoSplitJob, CcoSplitSource,
    CcoSplitStatus, CcoSplitTask, CcoTaskKind, CcoTaskStatus, OnFailure, PlanIR, TaskIR, TaskRole,
    TaskScope, CCO_SPLIT_SCHEMA, INSPECT_DEFAULT_ALLOWED_TOOLS, INSPECT_DEFAULT_WRITE_SCOPE,
    INSPECT_SYSTEM_PROMPT, INSPECT_SYSTEM_PROMPT_MARKER, MAX_PROMPT_CHARS, MAX_TASKS,
    MAX_TIMEOUT_SECS, PLANNER_MAX_BUDGET_USD, PLANNER_MAX_TASKS, SYS_POST_GIT_PUSH_ID,
    SYS_POST_INSPECT_ID, SYS_POST_OPEN_PR_ID,
};

// System post inject (config-aware host side).
pub use system_post::inject_system_post_tasks;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;

pub fn is_structured_adapter(adapter: &str) -> bool {
    matches!(adapter, "cco-plan/v1" | "serial-prompts/v0")
}

/// Peek adapter without full PlanIR parse (for CLI `run` routing).
pub fn peek_adapter(project_root: &Path, plan_path: &Path) -> Result<String> {
    let abs = resolve_plan_path(project_root, plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    Ok(detect_adapter(&abs, &text))
}

/// Detect adapter and load PlanIR.
pub fn load_plan(
    project_root: &Path,
    plan_path: &Path,
    adapter_hint: Option<&str>,
    config: &Config,
) -> Result<PlanIR> {
    let abs = resolve_plan_path(project_root, plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;

    let adapter = adapter_hint
        .map(|s| s.to_string())
        .unwrap_or_else(|| detect_adapter(&abs, &text));

    let mut plan = match adapter.as_str() {
        "cco-plan/v1" => adapters::cco_v1::parse(&abs, &text, config)?,
        "serial-prompts/v0" => adapters::serial_prompts::parse(&abs, &text, config)?,
        "raw-single" => adapters::raw_single::parse(&abs, &text, config)?,
        other => bail!("unknown adapter: {other}"),
    };
    plan.adapter = adapter;
    plan.source_path = abs;
    // P2-4: tags → soft provider hints (never overrides explicit non-default provider).
    apply_tag_routing(&mut plan);
    // P2-1: role=inspect default opts/scope before validate (require_inspect gate).
    materialize_role_defaults(&mut plan);
    plan.validate()?;
    Ok(plan)
}

pub fn resolve_plan_path(project_root: &Path, plan_path: &Path) -> Result<PathBuf> {
    let p = if plan_path.is_absolute() {
        plan_path.to_path_buf()
    } else {
        project_root.join(plan_path)
    };
    let canon = p
        .canonicalize()
        .with_context(|| format!("plan path not found: {}", p.display()))?;
    Ok(canon)
}

fn detect_adapter(path: &Path, text: &str) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let trimmed = text.trim_start();

    // YAML/JSON: only these are machine plan schemas (optional, not default).
    if matches!(ext.as_str(), "yaml" | "yml" | "json") {
        if text.contains("cco-plan/v1") || trimmed.starts_with("schema:") {
            return "cco-plan/v1".into();
        }
    }

    // Markdown with YAML frontmatter at the *start* only (not body examples).
    if trimmed.starts_with("---") {
        if let Some(rest) = trimmed.strip_prefix("---") {
            if let Some(end) = rest.find("\n---") {
                let front = &rest[..end];
                if front.contains("schema: cco-plan/v1") {
                    return "cco-plan/v1".into();
                }
            }
        }
    }

    // Pure yaml document starting with schema (no extension / .txt edge cases)
    if trimmed.starts_with("schema: cco-plan/v1") {
        return "cco-plan/v1".into();
    }

    // Default for documents: md (and anything else) is a plan *document*.
    // - multi-task prompt sections → serial-prompts/v0
    // - otherwise whole file is one prompt → raw-single
    // Never treat "schema: cco-plan/v1" appearing mid-document as schema.
    if text.contains("并行组")
        || text.contains("| id |")
        || (text.contains("## Tasks") && text.contains("### "))
    {
        return "serial-prompts/v0".into();
    }
    "raw-single".into()
}

/// List candidate plan files under project.
pub fn list_plans(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let candidates = [
        project_root.join("plans"), // chat-plan-builder 落盘优先目录
        project_root.join("docs/serial-plans"),
        project_root.join("docs/plans"),
        project_root.join("docs"),
        project_root.join(".cco"),
    ];
    for dir in candidates {
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir_shallow(&dir, 3)? {
            let name = entry
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.ends_with(".md")
                || name.ends_with(".yaml")
                || name.ends_with(".yml")
                || name == "plan.md"
            {
                // plans/ 下全部 .md 都算计划；其它目录仍要求文件名含 plan/prompt
                let under_plans = dir.ends_with("plans") || dir.ends_with("serial-plans");
                if name.contains("plan")
                    || name.contains("prompt")
                    || name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || under_plans
                {
                    out.push(entry);
                }
            }
        }
    }
    // 项目根 cco-plan-*.md（无 plans/ 时聊天落盘）
    if let Ok(rd) = std::fs::read_dir(project_root) {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_file() {
                continue;
            }
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.starts_with("cco-plan-") && name.ends_with(".md") {
                out.push(p);
            }
        }
    }
    out.sort();
    out.dedup();
    // Prefer markdown plan documents first; yaml/json are optional structured forms.
    out.sort_by(|a, b| {
        let rank = |p: &Path| {
            match p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "md" => 0,
                "yaml" | "yml" => 1,
                "json" => 2,
                _ => 3,
            }
        };
        rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
    });
    Ok(out)
}

fn walkdir_shallow(root: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn rec(dir: &Path, depth: usize, max: usize, out: &mut Vec<PathBuf>) -> Result<()> {
        if depth > max {
            return Ok(());
        }
        for ent in std::fs::read_dir(dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.is_dir() {
                rec(&p, depth + 1, max, out)?;
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    rec(root, 1, max_depth, &mut out)?;
    Ok(out)
}


#[cfg(test)]
mod tests {
    include!("plan_tests.rs");
}
