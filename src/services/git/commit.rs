//! Git commit: forbidden-pattern filtering, dry-run, and actual commit.
//!
//! [INPUT]: GitConfig · project path · message · flags
//! [OUTPUT]: CommitResult

use std::path::Path;

use anyhow::{bail, Result};

use super::*;
use crate::config::{Config, GitConfig};

/// Patterns that must never be `git add`-ed automatically.
const FORBIDDEN_PATTERNS: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    "id_rsa",
    "id_ed25519",
    ".pem",
    ".key",
    ".pfx",
    ".keystore",
];

fn is_forbidden_path(path: &str) -> bool {
    let p = path.trim();
    FORBIDDEN_PATTERNS.iter().any(|f| p.contains(f))
}

/// Filter forbidden files out of a list of paths. Returns (allowed, rejected).
fn filter_forbidden(paths: &[String]) -> (Vec<String>, Vec<String>) {
    let mut allowed = vec![];
    let mut rejected = vec![];
    for p in paths {
        if is_forbidden_path(p) {
            rejected.push(p.clone());
        } else {
            allowed.push(p.clone());
        }
    }
    (allowed, rejected)
}

/// List files that would be committed (staged + unstaged + untracked, deduped).
pub fn list_commit_candidates(project: &Path) -> Result<Vec<String>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let porcelain = git_run(project, &["status", "--porcelain"])?;
    let mut files: Vec<String> = vec![];
    for line in porcelain.lines() {
        let path_part = if let Some(idx) = line.find(" -> ") {
            &line[idx + 4..]
        } else {
            &line[3..]
        };
        let p = path_part.trim().trim_matches('"').to_string();
        if !p.is_empty() {
            files.push(p);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Commit changes in a project.
pub fn commit(
    config: &Config,
    project: &Path,
    message: &str,
    dry_run: bool,
    push: bool,
    all: bool,
    paths: &[String],
    force: bool,
) -> Result<CommitResult> {
    commit_with_git_config(
        &config.git,
        project,
        message,
        dry_run,
        push,
        all,
        paths,
        force,
    )
}

/// Commit using a Git policy snapshot without loading the global Config.
pub fn commit_with_git_config(
    git: &GitConfig,
    project: &Path,
    message: &str,
    dry_run: bool,
    push: bool,
    all: bool,
    paths: &[String],
    force: bool,
) -> Result<CommitResult> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    if message.trim().is_empty() {
        bail!("commit message cannot be empty");
    }

    let candidates = if all {
        list_commit_candidates(project)?
    } else {
        paths.to_vec()
    };
    let (allowed, rejected) = filter_forbidden(&candidates);
    let branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();

    if dry_run {
        return Ok(CommitResult {
            ok: true,
            message: format!(
                "dry-run: {} files would be added, {} rejected",
                allowed.len(),
                rejected.len()
            ),
            commit_hash: None,
            branch,
            files: allowed,
            pushed: false,
            push_output: None,
        });
    }

    if allowed.is_empty() {
        return Ok(CommitResult {
            ok: true,
            message: "no changes to commit (allowed files empty)".into(),
            commit_hash: None,
            branch,
            files: vec![],
            pushed: false,
            push_output: None,
        });
    }

    let mut add_args: Vec<String> = vec!["add".into(), "--".into()];
    add_args.extend(allowed.iter().cloned());
    let add_refs: Vec<&str> = add_args.iter().map(|s| s.as_str()).collect();
    git_run(project, &add_refs)?;

    let _commit_out = git_run(project, &["commit", "-m", message])?;
    let hash = git_run(project, &["rev-parse", "HEAD"]).ok();

    let mut pushed = false;
    let mut push_output: Option<String> = None;
    if push {
        let force_allowed = git.auto_commit.allow_force && force;
        let explicit_remote = (!git.auto_commit.push_remote.trim().is_empty())
            .then_some(git.auto_commit.push_remote.as_str());
        let explicit_branch = (!git.auto_commit.push_branch.trim().is_empty())
            .then_some(git.auto_commit.push_branch.as_str());
        match super::push::push_internal(
            git,
            project,
            explicit_remote,
            explicit_branch,
            force_allowed,
        ) {
            Ok(pr) => {
                pushed = pr.ok;
                push_output = Some(pr.message);
            }
            Err(e) => {
                push_output = Some(format!("push failed: {e}"));
            }
        }
    }

    Ok(CommitResult {
        ok: true,
        message: format!(
            "committed {} files ({} rejected){}",
            allowed.len(),
            rejected.len(),
            if rejected.is_empty() {
                String::new()
            } else {
                format!(": rejected {}", rejected.join(", "))
            }
        ),
        commit_hash: hash,
        branch,
        files: allowed,
        pushed,
        push_output,
    })
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
    fn forbidden_patterns_filtered() {
        assert!(is_forbidden_path(".env"));
        assert!(is_forbidden_path("secrets/id_rsa"));
        assert!(is_forbidden_path("cert.pem"));
        assert!(!is_forbidden_path("src/main.rs"));
        assert!(!is_forbidden_path("README.md"));
    }

    #[test]
    fn list_commit_candidates_includes_untracked() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("new.txt"), "x\n").unwrap();
        let files = list_commit_candidates(&root).unwrap();
        assert!(files.iter().any(|f| f == "new.txt"));
    }

    #[test]
    fn commit_dry_run_does_not_commit() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("b.txt"), "b\n").unwrap();
        let cfg = Config::default();
        let r = commit(&cfg, &root, "test", true, false, true, &[], false).unwrap();
        assert!(r.ok);
        assert!(r.commit_hash.is_none());
        assert!(r.files.iter().any(|f| f == "b.txt"));
        let log = git_run(&root, &["log", "--oneline"]).unwrap();
        assert_eq!(log.lines().count(), 1);
    }

    #[test]
    fn commit_actually_commits() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("c.txt"), "c\n").unwrap();
        let cfg = Config::default();
        let r = commit(&cfg, &root, "add c", false, false, true, &[], false).unwrap();
        assert!(r.ok);
        assert!(r.commit_hash.is_some());
        assert!(!r.pushed);
        let log = git_run(&root, &["log", "--oneline"]).unwrap();
        assert!(log.contains("add c"));
    }

    #[test]
    fn commit_push_uses_auto_commit_remote_and_branch() {
        let (_d, root) = make_test_repo().unwrap();
        let bare = tempfile::tempdir().unwrap();
        git_run(bare.path(), &["init", "--bare"]).unwrap();
        let remote_path = bare.path().to_str().unwrap();
        git_run(&root, &["remote", "add", "mirror", remote_path]).unwrap();
        fs::write(root.join("pushed.txt"), "pushed\n").unwrap();

        let mut cfg = Config::default();
        cfg.git.auto_commit.push_remote = "mirror".into();
        cfg.git.auto_commit.push_branch = "main".into();
        let result = commit_with_git_config(
            &cfg.git,
            &root,
            "push configured target",
            false,
            true,
            true,
            &[],
            false,
        )
        .unwrap();

        assert!(result.pushed, "{}", result.push_output.unwrap_or_default());
        let local_hash = result.commit_hash.unwrap();
        let remote_hash = git_run(bare.path(), &["rev-parse", "refs/heads/main"]).unwrap();
        assert_eq!(remote_hash, local_hash);
    }

    #[test]
    fn commit_rejects_forbidden() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        fs::write(root.join("ok.txt"), "ok\n").unwrap();
        let cfg = Config::default();
        let r = commit(&cfg, &root, "test", true, false, true, &[], false).unwrap();
        assert!(r.files.iter().any(|f| f == "ok.txt"));
        assert!(!r.files.iter().any(|f| f == ".env"));
    }
}
