//! Git log: commit history.
//!
//! [INPUT]: project path · max count
//! [OUTPUT]: Vec<LogEntry>

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// Get the last `n` log entries (default 20, max 200).
pub fn log(project: &Path, n: Option<usize>) -> Result<Vec<LogEntry>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let n = n.unwrap_or(20).min(200).max(1);
    let out = git_run(
        project,
        &[
            "log",
            &format!("-{n}"),
            "--format=%H|%an|%ad|%s",
            "--date=short",
        ],
    )?;
    let mut entries: Vec<LogEntry> = vec![];
    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 4 {
            continue;
        }
        entries.push(LogEntry {
            hash: parts[0].to_string(),
            author: parts[1].to_string(),
            date: parts[2].to_string(),
            message: parts[3].to_string(),
        });
    }
    Ok(entries)
}

/// Get one-line log output.
pub fn log_oneline(project: &Path, n: Option<usize>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let n = n.unwrap_or(20).min(200).max(1);
    git_run(project, &["log", &format!("-{n}"), "--oneline"])
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
    fn log_returns_entries() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("a.txt"), "a\n").unwrap();
        git_run(&root, &["add", "a.txt"]).unwrap();
        git_run(&root, &["commit", "-m", "second"]).unwrap();
        let entries = log(&root, Some(5)).unwrap();
        assert!(entries.len() >= 2);
        assert_eq!(entries[0].message, "second");
    }

    #[test]
    fn log_oneline_returns_output() {
        let (_d, root) = make_test_repo().unwrap();
        let out = log_oneline(&root, Some(5)).unwrap();
        assert!(out.contains("init"));
    }
}
