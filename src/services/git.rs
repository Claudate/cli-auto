//! Git service: host-level git operations (status / remote / config / commit / push / doctor).
//!
//! [INPUT]: Config.git · project path · CLI args
//! [OUTPUT]: GitStatusView · GitRemoteView · commit/push results · doctor lines
//! [POS]: services 子模块；薄封装 `git` CLI，无业务策略
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//!
//! 安全规则（与 system_post.rs 一致）：
//! - 禁止 force-push（除非 config.git.auto_commit.allow_force 且用户显式 --force）
//! - 禁止改全局 git config；identity 只设本仓库 --local
//! - 鉴权/冲突失败 → 停止并返回错误，不循环重试
//! - 不 add 密钥/.env/大二进制；commit 前可 dry-run 列出将提交文件

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::config::{region_label, Config, GitRegion, GitRemote};

/// Whether `project_root` is a git work tree.
pub fn is_git_repo(project_root: &Path) -> bool {
    project_root.join(".git").exists()
        || Command::new("git")
            .args(["-C"])
            .arg(project_root)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

/// Ensure `git` binary is available.
fn ensure_git_bin() -> Result<()> {
    if which::which("git").is_err() {
        bail!("git not found in PATH; install git or add to PATH");
    }
    Ok(())
}

/// Run `git -C <root> <args...>` and return stdout (trimmed). Errors carry stderr.
fn git_run(root: &Path, args: &[&str]) -> Result<String> {
    ensure_git_bin()?;
    let out = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            bail!("git {} failed (exit {})", args.join(" "), out.status);
        }
        bail!("{err}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// One-line view of a configured remote (config-side, not git-side).
#[derive(Debug, Clone, Serialize)]
pub struct GitRemoteView {
    pub name: String,
    pub url: String,
    pub region: String,
    pub region_label: String,
    pub note: Option<String>,
}

/// A remote as git actually sees it (`git remote -v`).
#[derive(Debug, Clone, Serialize)]
pub struct GitActualRemote {
    pub name: String,
    pub url: String,
    /// Whether this remote name matches a config.git.remotes entry.
    pub configured: bool,
}

/// Status snapshot for a project.
#[derive(Debug, Clone, Serialize)]
pub struct GitStatusView {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub clean: bool,
    /// `git status --porcelain` lines (staged + unstaged + untracked).
    pub changes: Vec<String>,
    /// Configured remotes (from config.git).
    pub configured_remotes: Vec<GitRemoteView>,
    /// Actual remotes (from `git remote -v`).
    pub actual_remotes: Vec<GitActualRemote>,
    /// Current user.name / user.email (repo-local if set, else global).
    pub user_name: Option<String>,
    pub user_email: Option<String>,
}

/// Result of a commit operation.
#[derive(Debug, Clone, Serialize)]
pub struct CommitResult {
    pub ok: bool,
    pub message: String,
    pub commit_hash: Option<String>,
    pub files: Vec<String>,
    pub pushed: bool,
    pub push_output: Option<String>,
}

/// Result of a push operation.
#[derive(Debug, Clone, Serialize)]
pub struct PushResult {
    pub ok: bool,
    pub message: String,
    pub remote: String,
    pub branch: String,
    pub output: Option<String>,
}

/// Doctor line for git (used by `cco git doctor` and desktop).
#[derive(Debug, Clone, Serialize)]
pub struct GitDoctorLine {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

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
    let user_name =
        git_run(project, &["config", "user.name"]).ok().filter(|s| !s.is_empty());
    let user_email =
        git_run(project, &["config", "user.email"]).ok().filter(|s| !s.is_empty());

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
        // format: "name\turl (fetch)"
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
/// Returns a list of human-readable actions taken.
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
            // update url
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
        // format: "XY path" or "XY path -> renamed"
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
///
/// - `message`: commit message (required).
/// - `dry_run`: if true, list candidates but do not add/commit.
/// - `push`: if true, push after successful commit (honors config.git.auto_commit policy).
/// - `all`: if true, `git add -A` (filtered); else only add listed `paths`.
/// - `paths`: explicit paths to add (ignored if `all`).
/// - `force`: allow force-push (only effective when `push` + config allows).
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
    if !is_git_repo(project) {
        bail!("not a git repository: {}", project.display());
    }
    if message.trim().is_empty() {
        bail!("commit message cannot be empty");
    }

    // Gather candidate files.
    let candidates = if all {
        list_commit_candidates(project)?
    } else {
        paths.to_vec()
    };
    let (allowed, rejected) = filter_forbidden(&candidates);

    if dry_run {
        return Ok(CommitResult {
            ok: true,
            message: format!(
                "dry-run: {} files would be added, {} rejected",
                allowed.len(),
                rejected.len()
            ),
            commit_hash: None,
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
            files: vec![],
            pushed: false,
            push_output: None,
        });
    }

    // git add allowed files
    let mut add_args: Vec<String> = vec!["add".into(), "--".into()];
    add_args.extend(allowed.iter().cloned());
    let add_refs: Vec<&str> = add_args.iter().map(|s| s.as_str()).collect();
    git_run(project, &add_refs)?;

    // git commit
    let _commit_out = git_run(project, &["commit", "-m", message])?;
    let hash = git_run(project, &["rev-parse", "HEAD"]).ok();

    let mut pushed = false;
    let mut push_output: Option<String> = None;
    if push {
        let force_allowed = config.git.auto_commit.allow_force && force;
        match push_internal(config, project, None, None, force_allowed) {
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
        files: allowed,
        pushed,
        push_output,
    })
}

/// Push current branch to a remote.
///
/// - `explicit_remote`: remote name; None → pick from config (default_region → first).
/// - `explicit_branch`: branch; None → current branch.
/// - `force`: force-push; only honored when config.git.auto_commit.allow_force is true.
pub fn push(
    config: &Config,
    project: &Path,
    explicit_remote: Option<&str>,
    explicit_branch: Option<&str>,
    force: bool,
) -> Result<PushResult> {
    let force_allowed = config.git.auto_commit.allow_force && force;
    push_internal(config, project, explicit_remote, explicit_branch, force_allowed)
}

fn push_internal(
    config: &Config,
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

    // Pick remote: explicit → config pick → actual first.
    let remote_name = if let Some(name) = explicit_remote.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        name.to_string()
    } else if let Some(r) = config.git.pick_push_remote(None) {
        r.name.clone()
    } else {
        // fall back to first actual remote
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
            actual.iter().map(|r| r.name.clone()).collect::<Vec<_>>().join(", ")
        },
    });

    // identity
    let name = git_run(project, &["config", "user.name"]).ok().filter(|s| !s.is_empty());
    let email = git_run(project, &["config", "user.email"]).ok().filter(|s| !s.is_empty());
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

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// Create a temp git repo with an initial commit.
    fn make_repo() -> Result<(tempfile::TempDir, PathBuf)> {
        let dir = tempdir()?;
        let root = dir.path().to_path_buf();
        // git init + identity + initial commit
        git_run(&root, &["init", "--initial-branch=main"])?;
        git_run(&root, &["config", "--local", "user.name", "test"])?;
        git_run(&root, &["config", "--local", "user.email", "test@example.com"])?;
        fs::write(root.join("README.md"), "# test\n")?;
        git_run(&root, &["add", "README.md"])?;
        git_run(&root, &["commit", "-m", "init"])?;
        Ok((dir, root))
    }

    #[test]
    fn is_git_repo_detects_init() {
        let (_d, root) = make_repo().unwrap();
        assert!(is_git_repo(&root));
    }

    #[test]
    fn status_reports_branch_and_clean() {
        let (_d, root) = make_repo().unwrap();
        let cfg = Config::default();
        let v = status(&cfg, &root).unwrap();
        assert!(v.is_repo);
        assert_eq!(v.branch.as_deref(), Some("main"));
        assert!(v.clean);
    }

    #[test]
    fn status_reports_changes() {
        let (_d, root) = make_repo().unwrap();
        fs::write(root.join("a.txt"), "hi\n").unwrap();
        let cfg = Config::default();
        let v = status(&cfg, &root).unwrap();
        assert!(!v.clean);
        assert!(v.changes.iter().any(|c| c.contains("a.txt")));
    }

    #[test]
    fn add_remote_persists_and_updates() {
        let mut cfg = Config::default();
        add_remote(&mut cfg, "gitee", "https://gitee.com/u/r.git", GitRegion::Domestic, None).unwrap();
        assert_eq!(cfg.git.remotes.len(), 1);
        // update same name
        add_remote(&mut cfg, "gitee", "https://gitee.com/u/r2.git", GitRegion::Domestic, Some("镜像".into())).unwrap();
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
        let (_d, root) = make_repo().unwrap();
        set_identity(&root, Some("alice"), Some("alice@example.com")).unwrap();
        let name = git_run(&root, &["config", "--local", "user.name"]).unwrap();
        assert_eq!(name, "alice");
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
        let (_d, root) = make_repo().unwrap();
        fs::write(root.join("new.txt"), "x\n").unwrap();
        let files = list_commit_candidates(&root).unwrap();
        assert!(files.iter().any(|f| f == "new.txt"));
    }

    #[test]
    fn commit_dry_run_does_not_commit() {
        let (_d, root) = make_repo().unwrap();
        fs::write(root.join("b.txt"), "b\n").unwrap();
        let cfg = Config::default();
        let r = commit(&cfg, &root, "test", true, false, true, &[], false).unwrap();
        assert!(r.ok);
        assert!(r.commit_hash.is_none());
        assert!(r.files.iter().any(|f| f == "b.txt"));
        // ensure not actually committed
        let log = git_run(&root, &["log", "--oneline"]).unwrap();
        assert_eq!(log.lines().count(), 1);
    }

    #[test]
    fn commit_actually_commits() {
        let (_d, root) = make_repo().unwrap();
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
    fn commit_rejects_forbidden() {
        let (_d, root) = make_repo().unwrap();
        fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        fs::write(root.join("ok.txt"), "ok\n").unwrap();
        let cfg = Config::default();
        let r = commit(&cfg, &root, "test", true, false, true, &[], false).unwrap();
        assert!(r.files.iter().any(|f| f == "ok.txt"));
        assert!(!r.files.iter().any(|f| f == ".env"));
    }

    #[test]
    fn doctor_reports_repo_and_identity() {
        let (_d, root) = make_repo().unwrap();
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
        let dir = tempdir().unwrap();
        let cfg = Config::default();
        let lines = doctor(&cfg, dir.path()).unwrap();
        let repo_line = lines.iter().find(|l| l.name == "git_repo").unwrap();
        assert!(!repo_line.ok);
    }
}
