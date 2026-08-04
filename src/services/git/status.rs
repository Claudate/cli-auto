//! Git status: repo status snapshot and remote listing.
//!
//! [INPUT]: Config.git · project root
//! [OUTPUT]: GitStatusView · Vec<GitActualRemote>

use std::path::Path;

use anyhow::Result;

use super::*;
use crate::config::{region_label, Config};

/// Get a full git status snapshot for a project.
pub fn status(config: &Config, project: &Path) -> Result<GitStatusView> {
    let is_repo = is_git_repo(project);
    if !is_repo {
        return Ok(GitStatusView {
            is_repo: false,
            branch: None,
            upstream: None,
            clean: true,
            changes: vec![],
            configured_remotes: configured_remote_views(config),
            actual_remotes: vec![],
            user_name: None,
            user_email: None,
        });
    }

    let branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    let upstream = git_run(project, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok();
    let porcelain = git_run(project, &["status", "--porcelain"]).unwrap_or_default();
    let changes: Vec<String> = porcelain
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let clean = changes.is_empty();

    let mut actual_remotes = list_actual_remotes(project)?;
    let configured_names: std::collections::HashSet<&str> =
        config.git.remotes.iter().map(|r| r.name.as_str()).collect();
    for r in &mut actual_remotes {
        r.configured = configured_names.contains(r.name.as_str());
    }
    let user_name = git_run(project, &["config", "user.name"])
        .ok()
        .filter(|s| !s.is_empty());
    let user_email = git_run(project, &["config", "user.email"])
        .ok()
        .filter(|s| !s.is_empty());

    Ok(GitStatusView {
        is_repo: true,
        branch,
        upstream,
        clean,
        changes,
        configured_remotes: configured_remote_views(config),
        actual_remotes,
        user_name,
        user_email,
    })
}

/// List actual remotes from `git remote -v` (deduped by name).
pub fn list_actual_remotes(project: &Path) -> Result<Vec<GitActualRemote>> {
    if !is_git_repo(project) {
        return Ok(vec![]);
    }
    let out = git_run(project, &["remote", "-v"]).unwrap_or_default();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut remotes: Vec<GitActualRemote> = vec![];
    for line in out.lines() {
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("");
        let url = parts.next().unwrap_or("");
        if name.is_empty() || url.is_empty() {
            continue;
        }
        if seen.insert(name.to_string()) {
            remotes.push(GitActualRemote {
                name: name.to_string(),
                url: url.to_string(),
                configured: false,
            });
        }
    }
    Ok(remotes)
}

/// Configured remotes as views (from config.git).
fn configured_remote_views(config: &Config) -> Vec<GitRemoteView> {
    config
        .git
        .remotes
        .iter()
        .map(|r| GitRemoteView {
            name: r.name.clone(),
            url: r.url.clone(),
            region: format!("{:?}", r.region).to_ascii_lowercase(),
            region_label: region_label(&r.region).to_string(),
            note: r.note.clone(),
        })
        .collect()
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
        git_run(
            &root,
            &["config", "--local", "user.email", "test@example.com"],
        )?;
        fs::write(root.join("README.md"), "# test\n")?;
        git_run(&root, &["add", "README.md"])?;
        git_run(&root, &["commit", "-m", "init"])?;
        Ok((dir, root))
    }

    #[test]
    fn is_git_repo_detects_init() {
        let (_d, root) = make_test_repo().unwrap();
        assert!(is_git_repo(&root));
    }

    #[test]
    fn status_reports_branch_and_clean() {
        let (_d, root) = make_test_repo().unwrap();
        let cfg = Config::default();
        let v = status(&cfg, &root).unwrap();
        assert!(v.is_repo);
        assert_eq!(v.branch.as_deref(), Some("main"));
        assert!(v.clean);
    }

    #[test]
    fn status_reports_changes() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("a.txt"), "hi\n").unwrap();
        let cfg = Config::default();
        let v = status(&cfg, &root).unwrap();
        assert!(!v.clean);
        assert!(v.changes.iter().any(|c| c.contains("a.txt")));
    }
}
