//! Git push: push current branch to remote with force-with-lease safety.
//!
//! [INPUT]: GitConfig · project path · remote/branch
//! [OUTPUT]: PushResult

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::*;
use crate::config::{Config, GitConfig};

/// Push current branch to a remote.
pub fn push(
    config: &Config,
    project: &Path,
    explicit_remote: Option<&str>,
    explicit_branch: Option<&str>,
    force: bool,
) -> Result<PushResult> {
    let force_allowed = config.git.auto_commit.allow_force && force;
    push_internal(
        &config.git,
        project,
        explicit_remote,
        explicit_branch,
        force_allowed,
    )
}

/// Push implementation (takes GitConfig snapshot to avoid borrowing Config).
pub fn push_internal(
    git: &GitConfig,
    project: &Path,
    explicit_remote: Option<&str>,
    explicit_branch: Option<&str>,
    force_allowed: bool,
) -> Result<PushResult> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let branch = match explicit_branch.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(b) => b.to_string(),
        None => git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"])?,
    };

    let remote_name =
        if let Some(name) = explicit_remote.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            name.to_string()
        } else if let Some(r) = git.pick_push_remote(None) {
            r.name.clone()
        } else {
            let actual = list_actual_remotes(project)?;
            if let Some(first) = actual.first() {
                first.name.clone()
            } else {
                bail!("no remote configured (set config.git.remotes or pass --remote)");
            }
        };

    let mut args: Vec<String> = vec!["push".into()];
    if force_allowed {
        args.push("--force-with-lease".into());
    }
    args.push(remote_name.clone());
    args.push(format!("refs/heads/{branch}:refs/heads/{branch}"));
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = Command::new("git")
        .args(["-C"])
        .arg(project)
        .args(refs)
        .output()
        .with_context(|| format!("git push {remote_name} {branch}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let combined = if stdout.is_empty() {
        stderr.clone()
    } else if stderr.is_empty() {
        stdout.clone()
    } else {
        format!("{stdout}\n{stderr}")
    };
    if !out.status.success() {
        bail!("git push failed: {combined}");
    }
    Ok(PushResult {
        ok: true,
        message: format!("pushed {branch} → {remote_name}"),
        remote: remote_name,
        branch,
        output: Some(combined),
    })
}
