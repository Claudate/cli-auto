//! Git remotes: config-side remote add/remove/apply + identity set.
//!
//! [INPUT]: &mut Config · project path
//! [OUTPUT]: add/remove bool · apply action list

use std::path::Path;

use anyhow::{bail, Result};

use super::*;
use crate::config::{Config, GitRegion, GitRemote};

/// Add a remote to config.git.remotes (idempotent by name; updates url/region if exists).
pub fn add_remote(
    config: &mut Config,
    name: &str,
    url: &str,
    region: GitRegion,
    note: Option<String>,
) -> Result<()> {
    if name.trim().is_empty() {
        bail!("remote name cannot be empty");
    }
    if url.trim().is_empty() {
        bail!("remote url cannot be empty");
    }
    if let Some(r) = config.git.remotes.iter_mut().find(|r| r.name == name) {
        r.url = url.to_string();
        r.region = region;
        r.note = note;
    } else {
        config.git.remotes.push(GitRemote {
            name: name.to_string(),
            url: url.to_string(),
            region,
            note,
        });
    }
    config.save()?;
    Ok(())
}

/// Remove a remote from config.git.remotes (does not touch git itself).
pub fn remove_remote(config: &mut Config, name: &str) -> Result<bool> {
    let before = config.git.remotes.len();
    config.git.remotes.retain(|r| r.name != name);
    let removed = config.git.remotes.len() != before;
    if removed {
        config.save()?;
    }
    Ok(removed)
}

/// Apply configured remotes to the actual git repo (`git remote add` / `set-url`).
/// Idempotent: existing remotes get url updated; missing ones added.
pub fn apply_remotes(config: &Config, project: &Path) -> Result<Vec<String>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let actual = list_actual_remotes(project)?;
    let actual_names: std::collections::HashSet<String> =
        actual.iter().map(|r| r.name.clone()).collect();
    let mut actions: Vec<String> = vec![];
    for r in &config.git.remotes {
        if actual_names.contains(&r.name) {
            git_run(project, &["remote", "set-url", &r.name, &r.url])?;
            actions.push(format!("set-url {} → {}", r.name, r.url));
        } else {
            git_run(project, &["remote", "add", &r.name, &r.url])?;
            actions.push(format!("add {} → {}", r.name, r.url));
        }
    }
    if actions.is_empty() {
        actions.push("no configured remotes to apply".into());
    }
    Ok(actions)
}

/// Set repo-local identity (`git config --local user.name/email`).
/// Never touches --global. Empty values clear the local setting.
pub fn set_identity(project: &Path, name: Option<&str>, email: Option<&str>) -> Result<()> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    if let Some(n) = name {
        if n.trim().is_empty() {
            git_run(project, &["config", "--local", "--unset", "user.name"])?;
        } else {
            git_run(project, &["config", "--local", "user.name", n])?;
        }
    }
    if let Some(e) = email {
        if e.trim().is_empty() {
            git_run(project, &["config", "--local", "--unset", "user.email"])?;
        } else {
            git_run(project, &["config", "--local", "user.email", e])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GitRegion;
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
    fn add_remote_persists_and_updates() {
        let mut cfg = Config::default();
        add_remote(
            &mut cfg,
            "gitee",
            "https://gitee.com/u/r.git",
            GitRegion::Domestic,
            None,
        )
        .unwrap();
        assert_eq!(cfg.git.remotes.len(), 1);
        add_remote(
            &mut cfg,
            "gitee",
            "https://gitee.com/u/r2.git",
            GitRegion::Domestic,
            Some("镜像".into()),
        )
        .unwrap();
        assert_eq!(cfg.git.remotes.len(), 1);
        assert_eq!(cfg.git.remotes[0].url, "https://gitee.com/u/r2.git");
        assert_eq!(cfg.git.remotes[0].note.as_deref(), Some("镜像"));
    }

    #[test]
    fn remove_remote_returns_bool() {
        let mut cfg = Config::default();
        add_remote(&mut cfg, "origin", "https://x", GitRegion::Overseas, None).unwrap();
        assert!(remove_remote(&mut cfg, "origin").unwrap());
        assert!(!remove_remote(&mut cfg, "origin").unwrap());
    }

    #[test]
    fn set_identity_local_only() {
        let (_d, root) = make_test_repo().unwrap();
        set_identity(&root, Some("alice"), Some("alice@example.com")).unwrap();
        let name = git_run(&root, &["config", "--local", "user.name"]).unwrap();
        assert_eq!(name, "alice");
    }
}
