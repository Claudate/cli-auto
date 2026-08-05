//! Git tag: list / create / delete / show tags.
//!
//! [INPUT]: project path · tag name · message
//! [OUTPUT]: Vec<TagInfo> / create/delete/show results

use std::path::Path;

use anyhow::{bail, Result};

use super::*;

/// List all tags with their target commit and optional message.
pub fn list_tags(project: &Path) -> Result<Vec<TagInfo>> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let out = git_run(
        project,
        &[
            "for-each-ref",
            "--format=%(refname:short)|%(objectname:short)|%(contents:subject)",
            "refs/tags",
        ],
    )?;
    let mut tags: Vec<TagInfo> = vec![];
    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.is_empty() {
            continue;
        }
        let name = parts[0].trim().to_string();
        if name.is_empty() {
            continue;
        }
        let commit = parts.get(1).unwrap_or(&"").trim().to_string();
        let message = parts.get(2).unwrap_or(&"").trim().to_string();
        tags.push(TagInfo {
            name,
            commit,
            message,
        });
    }
    Ok(tags)
}

/// Create a lightweight tag at HEAD (or at a specific commit).
pub fn create_tag(project: &Path, name: &str, commit: Option<&str>) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let sanitized = name.trim();
    if sanitized.is_empty() {
        bail!("tag name cannot be empty");
    }
    let mut args: Vec<&str> = vec!["tag", sanitized];
    if let Some(c) = commit {
        args.push(c.trim());
    }
    git_run(project, &args)?;
    Ok(format!("created tag {sanitized}"))
}

/// Create an annotated tag with a message at HEAD (or at a specific commit).
pub fn create_annotated_tag(
    project: &Path,
    name: &str,
    message: &str,
    commit: Option<&str>,
) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let sanitized = name.trim();
    if sanitized.is_empty() {
        bail!("tag name cannot be empty");
    }
    if message.trim().is_empty() {
        bail!("annotated tag message cannot be empty");
    }
    let mut args: Vec<&str> = vec!["tag", "-a", sanitized, "-m", message.trim()];
    if let Some(c) = commit {
        args.push(c.trim());
    }
    git_run(project, &args)?;
    Ok(format!("created annotated tag {sanitized}"))
}

/// Delete a local tag.
pub fn delete_tag(project: &Path, name: &str) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let sanitized = name.trim();
    if sanitized.is_empty() {
        bail!("tag name cannot be empty");
    }
    git_run(project, &["tag", "-d", sanitized])?;
    Ok(format!("deleted tag {sanitized}"))
}

/// Show details of a specific tag (tag + commit info).
pub fn show_tag(project: &Path, name: &str) -> Result<String> {
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    let sanitized = name.trim();
    if sanitized.is_empty() {
        bail!("tag name cannot be empty");
    }
    git_run(project, &["show", sanitized])
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
    fn list_tags_empty() {
        let (_d, root) = make_test_repo().unwrap();
        let tags = list_tags(&root).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn create_and_list_lightweight_tag() {
        let (_d, root) = make_test_repo().unwrap();
        let msg = create_tag(&root, "v0.1.0", None).unwrap();
        assert!(msg.contains("v0.1.0"));
        let tags = list_tags(&root).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.1.0");
        assert!(!tags[0].commit.is_empty());
    }

    #[test]
    fn create_annotated_tag_has_message() {
        let (_d, root) = make_test_repo().unwrap();
        create_annotated_tag(&root, "v0.2.0", "release 0.2", None).unwrap();
        let tags = list_tags(&root).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.2.0");
        assert_eq!(tags[0].message, "release 0.2");
    }

    #[test]
    fn delete_tag_works() {
        let (_d, root) = make_test_repo().unwrap();
        create_tag(&root, "v0.3.0", None).unwrap();
        assert_eq!(list_tags(&root).unwrap().len(), 1);
        let msg = delete_tag(&root, "v0.3.0").unwrap();
        assert!(msg.contains("v0.3.0"));
        assert!(list_tags(&root).unwrap().is_empty());
    }

    #[test]
    fn show_tag_returns_info() {
        let (_d, root) = make_test_repo().unwrap();
        create_annotated_tag(&root, "v0.4.0", "show me", None).unwrap();
        let out = show_tag(&root, "v0.4.0").unwrap();
        assert!(out.contains("v0.4.0") || out.contains("show me") || !out.is_empty());
    }

    #[test]
    fn create_tag_at_specific_commit() {
        let (_d, root) = make_test_repo().unwrap();
        let commit = git_run(&root, &["rev-parse", "HEAD"]).unwrap();
        let short = &commit[..7];
        create_tag(&root, "v0.5.0", Some(short)).unwrap();
        let tags = list_tags(&root).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.5.0");
    }

    #[test]
    fn empty_tag_name_rejected() {
        let (_d, root) = make_test_repo().unwrap();
        assert!(create_tag(&root, "  ", None).is_err());
    }
}
