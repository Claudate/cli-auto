//! Git doctor: diagnostic checks for git environment.
//!
//! [INPUT]: Config.git · project path
//! [OUTPUT]: Vec<GitDoctorLine>

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use super::*;
use crate::config::Config;

/// Git doctor lines for a project.
pub fn doctor(config: &Config, project: &Path) -> Result<Vec<GitDoctorLine>> {
    let mut lines: Vec<GitDoctorLine> = vec![];

    // git binary
    match which::which("git") {
        Ok(p) => lines.push(GitDoctorLine {
            name: "git_bin".into(),
            ok: true,
            detail: p.display().to_string(),
        }),
        Err(_) => lines.push(GitDoctorLine {
            name: "git_bin".into(),
            ok: false,
            detail: "git not in PATH".into(),
        }),
    }

    // is repo
    let is_repo = is_git_repo(project);
    lines.push(GitDoctorLine {
        name: "git_repo".into(),
        ok: is_repo,
        detail: if is_repo {
            "repository detected".into()
        } else {
            format!("not a git repo: {}", project.display())
        },
    });

    if !is_repo {
        return Ok(lines);
    }

    // branch
    let branch = git_run(project, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    lines.push(GitDoctorLine {
        name: "git_branch".into(),
        ok: branch.is_some(),
        detail: branch.clone().unwrap_or_else(|| "detached HEAD".into()),
    });

    // upstream
    let upstream = git_run(project, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok();
    lines.push(GitDoctorLine {
        name: "git_upstream".into(),
        ok: upstream.is_some(),
        detail: upstream.clone().unwrap_or_else(|| "no upstream set".into()),
    });

    // remotes
    let actual = list_actual_remotes(project)?;
    lines.push(GitDoctorLine {
        name: "git_remotes".into(),
        ok: !actual.is_empty(),
        detail: if actual.is_empty() {
            "no remotes configured".into()
        } else {
            actual
                .iter()
                .map(|r| r.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        },
    });

    // identity
    let name = git_run(project, &["config", "user.name"])
        .ok()
        .filter(|s| !s.is_empty());
    let email = git_run(project, &["config", "user.email"])
        .ok()
        .filter(|s| !s.is_empty());
    let identity_ok = name.is_some() && email.is_some();
    lines.push(GitDoctorLine {
        name: "git_identity".into(),
        ok: identity_ok,
        detail: match (&name, &email) {
            (Some(n), Some(e)) => format!("{n} <{e}>"),
            (Some(n), None) => format!("{n} (no email)"),
            (None, Some(e)) => format!("(no name) <{e}>"),
            (None, None) => "no user.name / user.email set".into(),
        },
    });

    // configured remotes vs actual
    let configured_names: std::collections::HashSet<String> =
        config.git.remotes.iter().map(|r| r.name.clone()).collect();
    let actual_names: std::collections::HashSet<String> =
        actual.iter().map(|r| r.name.clone()).collect();
    let unapplied: Vec<String> = configured_names
        .iter()
        .filter(|n| !actual_names.contains(*n))
        .cloned()
        .collect();
    if !unapplied.is_empty() {
        lines.push(GitDoctorLine {
            name: "git_remotes_unapplied".into(),
            ok: false,
            detail: format!("configured but not in repo: {}", unapplied.join(", ")),
        });
    }

    // gh binary and auth
    match which::which("gh") {
        Ok(p) => {
            lines.push(GitDoctorLine {
                name: "gh_bin".into(),
                ok: true,
                detail: p.display().to_string(),
            });
            let out = Command::new("gh")
                .args(["auth", "status"])
                .output()
                .context("gh auth status")?;
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            lines.push(GitDoctorLine {
                name: "gh_auth".into(),
                ok: out.status.success(),
                detail: if detail.is_empty() {
                    if out.status.success() {
                        "authenticated".into()
                    } else {
                        "not authenticated; run gh auth login".into()
                    }
                } else {
                    detail
                        .lines()
                        .next()
                        .unwrap_or("gh auth status")
                        .to_string()
                },
            });
        }
        Err(_) => lines.push(GitDoctorLine {
            name: "gh_bin".into(),
            ok: false,
            detail: "gh not in PATH; install GitHub CLI".into(),
        }),
    }

    Ok(lines)
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
    fn doctor_reports_repo_and_identity() {
        let (_d, root) = make_test_repo().unwrap();
        let cfg = Config::default();
        let lines = doctor(&cfg, &root).unwrap();
        let names: Vec<&str> = lines.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"git_bin"));
        assert!(names.contains(&"git_repo"));
        assert!(names.contains(&"git_identity"));
        let id = lines.iter().find(|l| l.name == "git_identity").unwrap();
        assert!(id.ok);
    }

    #[test]
    fn doctor_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::default();
        let lines = doctor(&cfg, dir.path()).unwrap();
        let repo_line = lines.iter().find(|l| l.name == "git_repo").unwrap();
        assert!(!repo_line.ok);
    }
}
