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
//! - `stash`    — 暂存/恢复
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
