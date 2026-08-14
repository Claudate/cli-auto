//! Git service: host-level git operations.
//!
//! [INPUT]: Config.git · project path · CLI args
//! [OUTPUT]: GitStatusView · GitRemoteView · commit/push/fetch/pull results · doctor lines
//! [POS]: services 子模块；薄封装 `git` CLI，无业务策略
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//!
//! 子模块：
//! - `status`   — status / remotes 查询
//! - `remotes`  — config 侧 remote 增删改
//! - `commit`   — commit / 禁传文件过滤
//! - `push`     — push / 冲突检测
//! - `pull`     — fetch / pull / 冲突策略 (merge|rebase|fail)
//! - `branch`   — 分支管理 (list/create/switch/delete)
//! - `log`      — 提交历史
//! - `diff`     — 差异对比
//! - `stash`    — 暂存/恢复/应用/丢弃/查看
//! - `tag`      — 标签管理 (list/create/delete/show)
//! - `doctor`   — 环境诊断
//!
//! 安全规则（与 system_post.rs 一致）：
//! - 禁止 force-push（除非 config.git.auto_commit.allow_force 且用户显式 --force）
//! - 禁止改全局 git config；identity 只设本仓库 --local
//! - 鉴权/冲突失败 → 停止并返回错误，不循环重试
//! - 不 add 密钥/.env/大二进制；commit 前可 dry-run 列出将提交文件

mod branch;
mod commit;
mod diff;
mod doctor;
mod log;
mod pull;
mod push;
mod remotes;
mod stash;
mod status;
mod tag;

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;

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

/// Gate: auto-commit only runs when the target project is a git repository.
///
/// Hard rule — 自动提交是可选能力：**没开不拦**；**开了必须校验通过才放行**。
/// Called once from `app::run::materialize::materialize_run_with_route_opts`
/// so both desktop confirm and CLI confirm share the same gate.
pub fn ensure_can_auto_commit(config: &crate::config::Config, project: &Path) -> Result<()> {
    use crate::config::AutoCommitGranularity;
    let granularity = config.auto_commit_granularity();
    if granularity == AutoCommitGranularity::Off {
        return Ok(());
    }
    if is_git_repo(project) {
        return Ok(());
    }
    bail!(
        "已开启自动提交（粒度：{}），但项目目录还不是 git 仓库：\n  {}\n\n\
        自动提交仅需本地 git，无需 GitHub 或远程仓库。请选择其一：\n  \
        1) 在该目录运行 `git init`（桌面拆分台提供「一键初始化」）\n  \
        2) 本次关闭自动提交后直接开始（桌面拆分台提供此选项）\n  \
        3) 永久关闭：设置 → 自动提交 → 关",
        granularity.as_str(),
        project.display()
    )
}

/// Initialize a fresh git repository at `project` (idempotent).
///
/// Used by the split desk "一键初始化 git" action when auto-commit is on but
/// the project is not yet a repo. After `git init`, applies any configured
/// repo-local identity and creates an initial commit so later worktree forks
/// have a HEAD to branch from.
pub fn init_repo(
    config: &crate::config::Config,
    project: &Path,
    default_branch: Option<&str>,
) -> Result<String> {
    if is_git_repo(project) {
        return Ok("already a git repository".into());
    }
    ensure_git_bin()?;
    let branch = default_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("main");
    git_run(project, &["init", "--initial-branch", branch])?;
    let id = &config.git.identity;
    if let Some(name) = id.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        git_run(project, &["config", "--local", "user.name", name])?;
    }
    if let Some(email) = id.email.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        git_run(project, &["config", "--local", "user.email", email])?;
    }
    // Initial commit so `git worktree add -b ... HEAD` has a base_ref.
    git_run(project, &["add", "-A"])?;
    // Allow empty tree (fresh project dirs may have nothing yet).
    git_run(project, &["commit", "--allow-empty", "-m", "chore: init"])?;
    Ok(format!("initialized git repo on branch '{branch}'"))
}

#[cfg(test)]
mod auto_commit_gate_tests {
    use super::*;
    use crate::config::AutoCommitGranularity;

    fn cfg_with(auto_commit: bool, granularity: AutoCommitGranularity) -> crate::config::Config {
        let mut c = crate::config::Config::default();
        c.git.auto_commit.enabled = auto_commit;
        c.git.auto_commit.granularity = granularity;
        c
    }

    /// 没开自动提交 → 不拦（即使非 git 目录）。
    #[test]
    fn gate_allows_when_auto_commit_off() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_with(false, AutoCommitGranularity::PerTask);
        ensure_can_auto_commit(&cfg, tmp.path()).unwrap();
    }

    /// 开了 + 项目已是 git 仓库 → 放行。
    #[test]
    fn gate_allows_when_project_is_repo() {
        let tmp = tempfile::tempdir().unwrap();
        git_run(tmp.path(), &["init", "--initial-branch", "main"]).unwrap();
        let cfg = cfg_with(true, AutoCommitGranularity::PerTask);
        ensure_can_auto_commit(&cfg, tmp.path()).unwrap();
    }

    /// 开了 + 项目还不是 git 仓库 → Err，文案含解决方案。
    #[test]
    fn gate_rejects_when_project_not_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_with(true, AutoCommitGranularity::PerTask);
        let err = ensure_can_auto_commit(&cfg, tmp.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("还不是 git 仓库"), "msg: {msg}");
        assert!(msg.contains("git init"), "should suggest git init: {msg}");
        assert!(msg.contains("永久关闭") || msg.contains("关闭自动提交"), "should mention turning off: {msg}");
    }

    /// init_repo on a fresh dir is idempotent-ish: second call short-circuits.
    #[test]
    fn init_repo_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = crate::config::Config::default();
        let first = init_repo(&cfg, tmp.path(), Some("main")).unwrap();
        assert!(first.contains("initialized"));
        assert!(is_git_repo(tmp.path()));
        let second = init_repo(&cfg, tmp.path(), Some("main")).unwrap();
        assert_eq!(second, "already a git repository");
    }
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

/// Run `git -C <root> <args...>` and return raw Output (for cases where
/// we want to inspect exit code without bailing).
fn git_run_raw(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    ensure_git_bin()?;
    Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))
}

// ── Shared types ──

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
    pub branch: Option<String>,
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

/// Result of a pull/fetch operation.
#[derive(Debug, Clone, Serialize)]
pub struct PullResult {
    pub ok: bool,
    pub message: String,
    pub files_changed: usize,
    pub branch: String,
    pub remote: String,
    pub merged: bool,
    pub output: Option<String>,
}

/// Result of a branch operation.
#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
}

/// Result of a stash operation.
#[derive(Debug, Clone, Serialize)]
pub struct StashEntry {
    pub index: usize,
    pub branch: String,
    pub message: String,
}

/// A single log entry.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// A single tag entry.
#[derive(Debug, Clone, Serialize)]
pub struct TagInfo {
    pub name: String,
    pub commit: String,
    pub message: String,
}

/// Doctor line for git (used by `cco git doctor` and desktop).
#[derive(Debug, Clone, Serialize)]
pub struct GitDoctorLine {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

// ── Re-exports ──

pub use branch::*;
pub use commit::*;
pub use diff::*;
pub use doctor::*;
pub use log::*;
pub use pull::*;
pub use push::*;
pub use remotes::*;
pub use stash::*;
pub use status::*;
pub use tag::*;
