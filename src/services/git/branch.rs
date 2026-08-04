//! Git branch: list / create / switch / delete branches.
//!
//! [INPUT]: project path
//! [OUTPUT]: Vec<BranchInfo> / create/switch/delete actions

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// List all local branches with current/upstream info.
pub fn list_branches(project: &Path) -> Result<Vec<BranchInfo>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let out = git_run(project, &["branch", "-vv"])?;
    let mut branches: Vec<BranchInfo> = vec![];
    for line in out.lines() {
        let trimmed = line.trim();
        let is_current = trimmed.starts_with("* ");
        let name_part = if is_current { &trimmed[2..] } else { trimmed };
        let name = name_part.split_whitespace().next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let upstream = line.find('[').and_then(|start| {
            let end = line[start..].find(']')?;
            let bracketed = &line[start + 1..start + end];
            let upstream_name = bracketed.split(':').next().unwrap_or(bracketed);
            if upstream_name.is_empty() { None } else { Some(upstream_name.to_string()) }
        });
        branches.push(BranchInfo { name, current: is_current, upstream });
    }
    Ok(branches)
}

/// Create a new branch from HEAD (or from a base ref).
pub fn create_branch(project: &Path, name: &str, base: Option<&str>) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    if name.trim().is_empty() { bail!("branch name cannot be empty"); }
    let sanitized = name.trim();
    let mut args: Vec<&str> = vec!["branch", sanitized];
    if let Some(base) = base { args.push(base); }
    git_run(project, &args)?;
    Ok(format!("created branch {sanitized}"))
}

/// Switch to an existing branch.
pub fn switch_branch(project: &Path, name: &str) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    if name.trim().is_empty() { bail!("branch name cannot be empty"); }
    git_run(project, &["checkout", name.trim()])?;
    let branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(format!("switched to {branch}"))
}

/// Delete a local branch (refuses to delete current branch unless force).
pub fn delete_branch(project: &Path, name: &str, force: bool) -> Result<String> {
    if !is_git_repo(project) { bail!("not a git repository: {}", project.display()); }
    if name.trim().is_empty() { bail!("branch name cannot be empty"); }
    let flag = if force { "-D" } else { "-d" };
    git_run(project, &["branch", flag, name.trim()])?;
    Ok(format!("deleted branch {}", name.trim()))
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
    fn list_initial_branch() {
        let (_d, root) = make_test_repo().unwrap();
        let branches = list_branches(&root).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].current);
    }

    #[test]
    fn create_and_switch_branch() {
        let (_d, root) = make_test_repo().unwrap();
        let msg = create_branch(&root, "feature-x", None).unwrap();
        assert!(msg.contains("feature-x"));
        let branches = list_branches(&root).unwrap();
        assert_eq!(branches.len(), 2);
        let msg = switch_branch(&root, "feature-x").unwrap();
        assert!(msg.contains("feature-x"));
        let branches = list_branches(&root).unwrap();
        let current = branches.iter().find(|b| b.current).unwrap();
        assert_eq!(current.name, "feature-x");
    }

    #[test]
    fn delete_branch_works() {
        let (_d, root) = make_test_repo().unwrap();
        create_branch(&root, "to-delete", None).unwrap();
        let msg = delete_branch(&root, "to-delete", false).unwrap();
        assert!(msg.contains("to-delete"));
        let branches = list_branches(&root).unwrap();
        assert_eq!(branches.len(), 1);
    }
}
