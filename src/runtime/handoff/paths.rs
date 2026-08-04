//! Path resolve · missing outputs · host CHANGED.md (A1-5 adapter IO).
//!
//! [INPUT]: TaskIR · work_dir · project_root
//! [OUTPUT]: resolved paths · missing list · optional CHANGED.md rel
//! [POS]: runtime/handoff — path join lives here, **not** domain
//! [PROTOCOL]: 输出相对路径形状勿静默改

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

use crate::plan::TaskIR;

use super::model::{role_str, scope_summary};

/// Resolve output path relative to work_dir, then project_root.
pub fn resolve_output_path(rel: &str, work_dir: &Path, project_root: &Path) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let in_work = work_dir.join(p);
    if in_work.exists() {
        return in_work;
    }
    project_root.join(p)
}

/// If TaskIR.outputs is non-empty, require each file to exist when task claims Done.
/// Returns Ok(missing) list (empty = all present). Empty outputs → Ok([]).
pub fn missing_outputs(task: &TaskIR, work_dir: &Path, project_root: &Path) -> Vec<String> {
    if task.outputs.is_empty() {
        return vec![];
    }
    task.outputs
        .iter()
        .filter(|o| {
            let path = resolve_output_path(o, work_dir, project_root);
            !path.is_file() && !path.is_dir()
        })
        .cloned()
        .collect()
}

/// Host-generated diff list for inspect (multi-cli P2-2).
///
/// Prefer `git status --short` / `git diff --name-status` in `work_dir` when it is a
/// git tree; otherwise list non-empty declared `outputs` that exist on disk.
/// Writes `.cco-out/<task_id>/CHANGED.md` under work_dir (fallback: project_root).
/// Returns the relative path written, or `None` if nothing useful to record.
pub fn write_task_diff(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Result<Option<String>> {
    let rel = format!(".cco-out/{}/CHANGED.md", task.id);
    let out_path = {
        let under_wd = work_dir.join(&rel);
        if work_dir.exists() {
            under_wd
        } else {
            project_root.join(&rel)
        }
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    let mut body = String::new();
    body.push_str(&format!("# CHANGED · {}\n\n", task.id));
    body.push_str(&format!("- provider: `{}`\n", task.provider));
    if let Some(role) = role_str(task.role) {
        body.push_str(&format!("- role: `{role}`\n"));
    }
    let scope = scope_summary(task);
    if !scope.is_empty() {
        body.push_str(&format!("- scope: `{scope}`\n"));
    }
    body.push_str(&format!("- work_dir: `{}`\n", work_dir.display()));
    body.push_str(&format!("- generated: {}\n\n", Utc::now().to_rfc3339()));

    let git_cwd = if work_dir.join(".git").exists() || is_git_worktree(work_dir) {
        work_dir
    } else if project_root.join(".git").exists() {
        project_root
    } else {
        work_dir
    };

    let mut lines: Vec<String> = Vec::new();
    if let Some(status) = git_capture(git_cwd, &["status", "--short"]) {
        for line in status.lines() {
            let t = line.trim();
            if !t.is_empty() {
                lines.push(t.to_string());
            }
        }
    }
    if lines.is_empty() {
        if let Some(diff) = git_capture(git_cwd, &["diff", "--name-status", "HEAD"]) {
            for line in diff.lines() {
                let t = line.trim();
                if !t.is_empty() {
                    lines.push(t.to_string());
                }
            }
        }
    }
    if lines.is_empty() {
        // Fallback: declared outputs that exist (no git available).
        for o in &task.outputs {
            let p = resolve_output_path(o, work_dir, project_root);
            if p.exists() {
                lines.push(format!("OUT {o}"));
            }
        }
    }

    body.push_str("## Files\n\n");
    if lines.is_empty() {
        body.push_str("_no git changes detected; declared outputs empty or missing_\n");
    } else {
        // Cap for inspect readability.
        const MAX: usize = 200;
        for (i, line) in lines.iter().enumerate() {
            if i >= MAX {
                body.push_str(&format!("\n… and {} more\n", lines.len() - MAX));
                break;
            }
            body.push_str(&format!("- `{line}`\n"));
        }
    }
    body.push_str("\n## Notes\n\n");
    body.push_str("- Host-generated for inspect (multi-cli P2-2). Workers should not overwrite.\n");
    body.push_str("- Prefer matching these paths against declared `scope.paths`.\n");

    std::fs::write(&out_path, body).with_context(|| format!("write {}", out_path.display()))?;
    Ok(Some(rel))
}

fn is_git_worktree(dir: &Path) -> bool {
    // worktree: .git is a file pointing at main repo
    dir.join(".git").is_file()
}

fn git_capture(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
