//! Git pull / fetch: fetch from remote + pull with merge/rebase strategies.
//!
//! [INPUT]: GitConfig · project path · remote/branch · PullStrategy
//! [OUTPUT]: PullResult
//!
//! 冲突处理策略（与 push 联动）：
//! - push 失败 → 检测是否因远端有新提交（non-fast-forward）
//! - 若是 → 按 PullStrategy 自动 fetch + merge/rebase 后再 push

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::*;
use crate::config::{Config, GitConfig};

/// How to handle conflicts / upstream divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullStrategy {
    Rebase,
    Merge,
    Fail,
}

impl Default for PullStrategy {
    fn default() -> Self { Self::Rebase }
}

impl PullStrategy {
    pub fn as_str(self) -> &'static str {
        match self { Self::Rebase => "rebase", Self::Merge => "merge", Self::Fail => "fail" }
    }
}

/// Fetch from a remote (or all remotes).
pub fn fetch(project: &Path, remote: Option<&str>, prune: bool) -> Result<PullResult> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    let mut args: Vec<&str> = vec!["fetch"];
    if prune { args.push("--prune"); }
    let remote_name = if let Some(r) = remote { args.push(r); r.to_string() } else { "origin".into() };
    let out = git_run_raw(project, &args)?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let combined = if stdout.is_empty() { stderr.clone() }
        else if stderr.is_empty() { stdout.clone() }
        else { format!("{stdout}\n{stderr}") };
    Ok(PullResult {
        ok: out.status.success(),
        message: format!("fetched from {remote_name}"),
        files_changed: 0,
        branch: branch.unwrap_or_default(),
        remote: remote_name,
        merged: false,
        output: Some(combined),
    })
}

/// Pull from a remote: fetch + merge/rebase according to strategy.
pub fn pull(
    config: &Config, project: &Path, remote: Option<&str>,
    branch: Option<&str>, strategy: PullStrategy,
) -> Result<PullResult> {
    pull_with_git_config(&config.git, project, remote, branch, strategy)
}

/// Pull using a GitConfig snapshot.
pub fn pull_with_git_config(
    git: &GitConfig, project: &Path, remote: Option<&str>,
    branch: Option<&str>, strategy: PullStrategy,
) -> Result<PullResult> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let current_branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let remote_name = resolve_remote_name(git, project, remote)?;
    let pull_branch = branch.map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or(&current_branch);
    match strategy {
        PullStrategy::Fail => {
            let fetch_result = fetch(project, Some(&remote_name), false)?;
            if !fetch_result.ok { bail!("fetch failed: {}", fetch_result.output.unwrap_or_default()); }
            let behind = count_behind(project, &remote_name, pull_branch)?;
            Ok(PullResult {
                ok: true,
                message: format!("fetched {remote_name}/{pull_branch} ({} commits behind; use --strategy rebase|merge to integrate)", behind),
                files_changed: 0, branch: current_branch, remote: remote_name, merged: false,
                output: Some(fetch_result.output.unwrap_or_default()),
            })
        }
        PullStrategy::Rebase => execute_pull_rebase(project, &remote_name, pull_branch),
        PullStrategy::Merge => execute_pull_merge(project, &remote_name, pull_branch),
    }
}

/// Try to push; if it fails due to non-fast-forward, auto-pull and retry.
pub fn push_with_conflict_resolution(
    config: &Config, project: &Path, remote: Option<&str>, branch: Option<&str>,
    force: bool, auto_pull_strategy: PullStrategy,
) -> Result<(PushResult, Option<PullResult>)> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    match push(config, project, remote, branch, force) {
        Ok(pr) => Ok((pr, None)),
        Err(e) => {
            let err_msg = e.to_string();
            if !is_non_fast_forward(&err_msg) || auto_pull_strategy == PullStrategy::Fail {
                return Err(e);
            }
            let pull_result = pull(config, project, remote, branch, auto_pull_strategy)?;
            if !pull_result.ok {
                bail!("push rejected (non-fast-forward) and auto-pull failed: {}", pull_result.message);
            }
            match push(config, project, remote, branch, force) {
                Ok(pr) => Ok((pr, Some(pull_result))),
                Err(e2) => bail!(
                    "push still failed after auto-pull: {e2}. Pull output: {}",
                    pull_result.output.unwrap_or_default()
                ),
            }
        }
    }
}

// ── Internal helpers ──

fn resolve_remote_name(git: &GitConfig, project: &Path, explicit: Option<&str>) -> Result<String> {
    if let Some(name) = explicit.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return Ok(name.to_string());
    }
    if let Some(r) = git.pick_push_remote(None) { return Ok(r.name.clone()); }
    let actual = list_actual_remotes(project)?;
    if let Some(first) = actual.first() { Ok(first.name.clone()) } else { bail!("no remote configured"); }
}

fn is_non_fast_forward(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("non-fast-forward") || lower.contains("updates were rejected")
        || lower.contains("failed to push") || lower.contains("the remote contains work")
}

fn count_behind(project: &Path, remote: &str, branch: &str) -> Result<usize> {
    let out = git_run_raw(project, &["rev-list", "--count", &format!("HEAD..{remote}/{branch}")])?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(s.parse::<usize>().unwrap_or(0))
    } else { Ok(0) }
}

fn execute_pull_rebase(project: &Path, remote: &str, branch: &str) -> Result<PullResult> {
    let current_branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let out = Command::new("git").args(["-C"]).arg(project)
        .args(["pull", "--rebase", remote, branch])
        .output().with_context(|| format!("git pull --rebase {remote} {branch}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let combined = if stdout.is_empty() { stderr.clone() }
        else if stderr.is_empty() { stdout.clone() }
        else { format!("{stdout}\n{stderr}") };
    if !out.status.success() { bail!("git pull --rebase failed: {combined}"); }
    let files = estimate_changed_files(&combined);
    Ok(PullResult {
        ok: true, message: format!("pulled {remote}/{branch} (rebase)"),
        files_changed: files, branch: current_branch, remote: remote.to_string(),
        merged: true, output: Some(combined),
    })
}

fn execute_pull_merge(project: &Path, remote: &str, branch: &str) -> Result<PullResult> {
    let current_branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let out = Command::new("git").args(["-C"]).arg(project)
        .args(["pull", "--no-rebase", remote, branch])
        .output().with_context(|| format!("git pull --no-rebase {remote} {branch}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let combined = if stdout.is_empty() { stderr.clone() }
        else if stderr.is_empty() { stdout.clone() }
        else { format!("{stdout}\n{stderr}") };
    if !out.status.success() { bail!("git pull (merge) failed: {combined}"); }
    let files = estimate_changed_files(&combined);
    Ok(PullResult {
        ok: true, message: format!("pulled {remote}/{branch} (merge)"),
        files_changed: files, branch: current_branch, remote: remote.to_string(),
        merged: true, output: Some(combined),
    })
}

fn estimate_changed_files(output: &str) -> usize {
    output.lines().filter(|l| l.contains('|')).count().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_repo() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().to_path_buf();
        git_run(&root, &["init", "--initial-branch=main"])?;
        git_run(&root, &["config", "--local", "user.name", "test"])?;
        git_run(&root, &["config", "--local", "user.email", "test@example.com"])?;
        fs::write(root.join("README.md"), "# test\n")?;
        git_run(&root, &["add", "README.md"])?;
        git_run(&root, &["commit", "-m", "init"])?;
        Ok((dir, root))
    }

    #[test]
    fn is_non_fast_forward_detects() {
        assert!(is_non_fast_forward("! [rejected] main -> main (non-fast-forward)"));
        assert!(is_non_fast_forward("Updates were rejected because the remote contains work"));
        assert!(!is_non_fast_forward("Permission denied"));
        assert!(!is_non_fast_forward("Everything up-to-date"));
    }

    #[test]
    fn push_with_conflict_resolution_no_remote() {
        let (_d, root) = make_test_repo().unwrap();
        let cfg = Config::default();
        let result = push_with_conflict_resolution(&cfg, &root, None, None, false, PullStrategy::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn pull_fail_strategy_fetches() {
        let (_d, root) = make_test_repo().unwrap();
        let bare = tempfile::tempdir().unwrap();
        git_run(bare.path(), &["init", "--bare"]).unwrap();
        let remote_path = bare.path().to_str().unwrap();
        git_run(&root, &["remote", "add", "origin", remote_path]).unwrap();
        git_run(&root, &["push", "origin", "main"]).unwrap();
        let cfg = Config::default();
        let result = pull(&cfg, &root, Some("origin"), Some("main"), PullStrategy::Fail).unwrap();
        assert!(result.ok);
        assert!(!result.merged);
    }
}
