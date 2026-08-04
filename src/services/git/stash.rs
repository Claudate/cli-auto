//! Git stash: save and restore working-tree changes.
//!
//! [INPUT]: project path · message
//! [OUTPUT]: stash entry list · push/pop results

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// Push current changes to stash. Returns the stash message.
pub fn stash_push(project: &Path, message: Option<&str>) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let mut args: Vec<&str> = vec!["stash", "push"];
    if let Some(msg) = message { args.push("-m"); args.push(msg); }
    let out = git_run(project, &args)?;
    if out.is_empty() || out.contains("No local changes") {
        Ok("no changes to stash".into())
    } else {
        Ok(out)
    }
}

/// Pop the latest stash entry.
pub fn stash_pop(project: &Path, index: Option<usize>) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let stash_ref = match index {
        Some(i) => format!("stash@{{{i}}}"),
        None => "stash@{0}".into(),
    };
    git_run(project, &["stash", "pop", &stash_ref])
}

/// List stash entries.
pub fn stash_list(project: &Path) -> Result<Vec<StashEntry>> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    let out = git_run(project, &["stash", "list"])?;
    let mut entries: Vec<StashEntry> = vec![];
    for (i, line) in out.lines().enumerate() {
        let rest = line.splitn(3, ':').nth(2).unwrap_or("").trim().to_string();
        entries.push(StashEntry {
            index: i,
            branch: String::new(),
            message: if rest.is_empty() { line.to_string() } else { rest },
        });
    }
    Ok(entries)
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
    fn stash_list_empty() {
        let (_d, root) = make_test_repo().unwrap();
        let entries = stash_list(&root).unwrap();
        assert!(entries.is_empty());
    }
}
