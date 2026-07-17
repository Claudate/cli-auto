//! Optional git worktree isolation per task.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub created: bool,
}

/// Whether `project_root` looks like a git work tree.
pub fn is_git_repo(project_root: &Path) -> bool {
    project_root.join(".git").exists()
        || Command::new("git")
            .args(["-C"])
            .arg(project_root)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

/// Create (or reuse) a worktree for a task.
///
/// Layout: `{project}/.cco-worktrees/cco-{run}-{task}/`
/// Branch: `cco/{run_id}/{task_id}`
pub fn ensure_worktree(
    project_root: &Path,
    run_id: &str,
    task_id: &str,
) -> Result<WorktreeInfo> {
    if which::which("git").is_err() {
        bail!("git not found; cannot create worktree");
    }
    if !is_git_repo(project_root) {
        bail!(
            "project is not a git repository: {}",
            project_root.display()
        );
    }

    let safe_run: String = run_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let safe_task: String = task_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();

    let branch = format!("cco/{safe_run}/{safe_task}");
    let path = project_root
        .join(".cco-worktrees")
        .join(format!("cco-{safe_run}-{safe_task}"));

    if path.exists() {
        info!(path = %path.display(), branch = %branch, "reusing existing worktree");
        return Ok(WorktreeInfo {
            path,
            branch,
            created: false,
        });
    }

    std::fs::create_dir_all(project_root.join(".cco-worktrees"))
        .context("create .cco-worktrees")?;

    // Prefer branching from HEAD
    let add = Command::new("git")
        .args(["-C"])
        .arg(project_root)
        .args(["worktree", "add", "-b"])
        .arg(&branch)
        .arg(&path)
        .arg("HEAD")
        .output()
        .context("git worktree add")?;

    if !add.status.success() {
        let err = String::from_utf8_lossy(&add.stderr);
        // Branch may already exist — try without -b
        if err.contains("already exists") {
            let add2 = Command::new("git")
                .args(["-C"])
                .arg(project_root)
                .args(["worktree", "add"])
                .arg(&path)
                .arg(&branch)
                .output()
                .context("git worktree add existing branch")?;
            if !add2.status.success() {
                bail!(
                    "git worktree add failed: {}",
                    String::from_utf8_lossy(&add2.stderr)
                );
            }
        } else {
            bail!("git worktree add failed: {err}");
        }
    }

    // Best-effort ignore file for main repo
    ensure_gitignore_entry(project_root, ".cco-worktrees/")?;

    info!(path = %path.display(), branch = %branch, "created worktree");
    Ok(WorktreeInfo {
        path,
        branch,
        created: true,
    })
}

/// Resolve work dir for a task: worktree path or project_root.
pub fn resolve_work_dir(
    project_root: &Path,
    run_id: &str,
    task_id: &str,
    want_worktree: bool,
) -> Result<(PathBuf, Option<WorktreeInfo>)> {
    if !want_worktree {
        return Ok((project_root.to_path_buf(), None));
    }
    match ensure_worktree(project_root, run_id, task_id) {
        Ok(info) => Ok((info.path.clone(), Some(info))),
        Err(e) => {
            warn!(error = %e, "worktree unavailable; using project_root");
            Ok((project_root.to_path_buf(), None))
        }
    }
}

fn ensure_gitignore_entry(project_root: &Path, entry: &str) -> Result<()> {
    let gi = project_root.join(".gitignore");
    if !gi.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&gi).unwrap_or_default();
    if text.lines().any(|l| l.trim() == entry || l.trim() == entry.trim_end_matches('/')) {
        return Ok(());
    }
    // Do not auto-modify user gitignore in M2 — only log
    tracing::debug!(entry, "consider adding to .gitignore");
    Ok(())
}

/// List worktrees under project (for report / cleanup hints).
pub fn list_cco_worktrees(project_root: &Path) -> Result<Vec<PathBuf>> {
    let root = project_root.join(".cco-worktrees");
    if !root.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for ent in std::fs::read_dir(root)? {
        let ent = ent?;
        if ent.file_type()?.is_dir() {
            out.push(ent.path());
        }
    }
    out.sort();
    Ok(out)
}
