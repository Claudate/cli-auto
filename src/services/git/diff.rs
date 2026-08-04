//! Git diff: show working-tree / staged / commit-to-commit diffs.
//!
//! [INPUT]: project path · options
//! [OUTPUT]: diff text / file list

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// Show diff of working tree (unstaged changes).
pub fn diff(project: &Path) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    git_run(project, &["diff"])
}

/// Show diff of staged changes.
pub fn diff_staged(project: &Path) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    git_run(project, &["diff", "--staged"])
}

/// Show diff between two commits (or HEAD~n).
pub fn diff_range(project: &Path, from: &str, to: &str) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    git_run(project, &["diff", from, to])
}

/// Show `git diff --stat` for working tree.
pub fn diff_stat(project: &Path) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    git_run(project, &["diff", "--stat"])
}

/// Show changed file names only.
pub fn diff_name_only(project: &Path) -> Result<Vec<String>> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let out = git_run(project, &["diff", "--name-only"])?;
    Ok(out.lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect())
}
