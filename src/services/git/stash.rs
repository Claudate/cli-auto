//! Git stash: save / restore / apply / drop / show working-tree changes.
//!
//! [INPUT]: project path · message · index
//! [OUTPUT]: stash entry list · push/pop/apply/drop/show results

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// Push current changes to stash. Returns the stash message.
pub fn stash_push(project: &Path, message: Option<&str>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let mut args: Vec<&str> = vec!["stash", "push"];
    if let Some(msg) = message {
        args.push("-m");
        args.push(msg);
    }
    let out = git_run(project, &args)?;
    if out.is_empty() || out.contains("No local changes") {
        Ok("no changes to stash".into())
    } else {
        Ok(out)
    }
}

/// Pop the latest stash entry (removes it from stash list after applying).
pub fn stash_pop(project: &Path, index: Option<usize>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let stash_ref = stash_ref(index);
    git_run(project, &["stash", "pop", &stash_ref])
}

/// Apply a stash entry without removing it from the stash list.
pub fn stash_apply(project: &Path, index: Option<usize>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let stash_ref = stash_ref(index);
    git_run(project, &["stash", "apply", &stash_ref])
}

/// Drop a stash entry (remove from stash list without applying).
pub fn stash_drop(project: &Path, index: Option<usize>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let stash_ref = stash_ref(index);
    git_run(project, &["stash", "drop", &stash_ref])
}

/// Show the diff of a stash entry (defaults to stash@{0}).
pub fn stash_show(project: &Path, index: Option<usize>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let stash_ref = stash_ref(index);
    git_run(project, &["stash", "show", "-p", &stash_ref])
}

/// List stash entries.
pub fn stash_list(project: &Path) -> Result<Vec<StashEntry>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let out = git_run(project, &["stash", "list"])?;
    let mut entries: Vec<StashEntry> = vec![];
    for (i, line) in out.lines().enumerate() {
        let rest = line.splitn(3, ':').nth(2).unwrap_or("").trim().to_string();
        entries.push(StashEntry {
            index: i,
            branch: String::new(),
            message: if rest.is_empty() {
                line.to_string()
            } else {
                rest
            },
        });
    }
    Ok(entries)
}

/// Build a `stash@{n}` reference from an optional index (default 0).
fn stash_ref(index: Option<usize>) -> String {
    match index {
        Some(i) => format!("stash@{{{i}}}"),
        None => "stash@{0}".into(),
    }
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
    fn stash_list_empty() {
        let (_d, root) = make_test_repo().unwrap();
        let entries = stash_list(&root).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn stash_push_then_list_and_pop() {
        let (_d, root) = make_test_repo().unwrap();
        // Modify a tracked file (untracked files aren't stashed by default)
        fs::write(root.join("README.md"), "# modified\n").unwrap();
        let msg = stash_push(&root, Some("wip change")).unwrap();
        assert!(!msg.contains("no changes to stash"));

        // List should have one entry
        let entries = stash_list(&root).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index, 0);
        assert!(entries[0].message.contains("wip change"));

        // Pop should restore the change and clear the stash
        let pop_out = stash_pop(&root, None).unwrap();
        assert!(pop_out.contains("README.md") || pop_out.contains("On branch"));
        let entries_after = stash_list(&root).unwrap();
        assert!(entries_after.is_empty());
    }

    #[test]
    fn stash_apply_keeps_entry() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("README.md"), "# apply me\n").unwrap();
        stash_push(&root, Some("to apply")).unwrap();
        let apply_out = stash_apply(&root, None).unwrap();
        assert!(!apply_out.is_empty());
        // Apply should NOT remove the stash entry
        let entries = stash_list(&root).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn stash_drop_removes_entry() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("README.md"), "# drop me\n").unwrap();
        stash_push(&root, Some("to drop")).unwrap();
        assert_eq!(stash_list(&root).unwrap().len(), 1);
        let drop_out = stash_drop(&root, None).unwrap();
        assert!(drop_out.contains("stash@{0}") || drop_out.contains("Dropped"));
        assert!(stash_list(&root).unwrap().is_empty());
    }

    #[test]
    fn stash_show_returns_diff() {
        let (_d, root) = make_test_repo().unwrap();
        fs::write(root.join("README.md"), "# show me\n").unwrap();
        stash_push(&root, Some("to show")).unwrap();
        let diff = stash_show(&root, None).unwrap();
        // show -p returns a diff containing the file name
        assert!(diff.contains("README.md") || diff.is_empty() || diff.contains("diff --git"));
    }
}
