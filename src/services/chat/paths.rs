//! Chat disk path helpers (adapter · not domain).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::domain::chat::sanitize_session_id;

pub(crate) fn chat_dir(project: &Path) -> PathBuf {
    project.join(".cco").join("chat")
}

pub(crate) fn session_path(project: &Path, session_id: &str) -> PathBuf {
    let safe = sanitize_session_id(session_id);
    chat_dir(project).join(format!("{safe}.json"))
}

/// Resolve a project-relative plan path; reject `..` / absolute escape.
pub(crate) fn resolve_project_plan_file(project: &Path, plan_rel: &str) -> Result<(String, PathBuf)> {
    let rel = plan_rel.trim().trim_start_matches('/');
    if rel.is_empty() {
        bail!("empty plan path");
    }
    if rel.contains("..") {
        bail!("plan path must not contain '..'");
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        bail!("plan path must be project-relative");
    }
    let project_canon = project
        .canonicalize()
        .with_context(|| format!("canonicalize project {}", project.display()))?;
    let abs = project_canon.join(rel_path);
    // Ensure still under project (even without existing file).
    if let Some(parent) = abs.parent().map(|p| {
        if p.exists() {
            p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
        } else {
            p.to_path_buf()
        }
    }) {
        let parent_s = parent.to_string_lossy();
        let root_s = project_canon.to_string_lossy();
        if parent_s != root_s && !parent_s.starts_with(root_s.as_ref()) {
            let prefix = format!("{}{}", root_s, std::path::MAIN_SEPARATOR);
            if !parent_s.starts_with(&prefix) && parent != project_canon {
                bail!("plan path escapes project root");
            }
        }
    }
    let rel_norm = rel.replace('\\', "/");
    Ok((rel_norm, abs))
}

/// Work dir for in-flight chat CLI turn (`__chat__` task).
pub(crate) fn chat_work_task_dir(project: &Path) -> PathBuf {
    project
        .join(".cco")
        .join("chat")
        .join("_work")
        .join("tasks")
        .join("__chat__")
}

/// Work dir for G0b normalize CLI turn.
pub(crate) fn normalize_work_task_dir(project: &Path) -> PathBuf {
    project
        .join(".cco")
        .join("chat")
        .join("_work")
        .join("tasks")
        .join("__normalize__")
}
