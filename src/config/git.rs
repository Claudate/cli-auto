//! Git configuration: remotes (国内/国外), identity, auto-commit policy.
//!
//! [INPUT]: config.toml `[git]` section
//! [OUTPUT]: GitConfig · GitRemote · GitIdentity · AutoCommitPolicy
//! [POS]: config 子模块；纯数据，无 IO
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/config/CLAUDE.md
//!
//! 设计：
//! - remotes: 命名 remote 列表（origin/gitee/github/…），每个含 url + region 标签
//! - identity: 可选 user.name / user.email（不强制覆盖本机全局）
//! - auto_commit: 自动提交策略（开关 + message 模板 + 是否 push）
//! - 所有字段 serde(default)，旧 config.toml 无 [git] 段时走默认值

use serde::{Deserialize, Serialize};

/// Region tag for a remote: domestic (国内) or overseas (国外).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GitRegion {
    /// 国内（Gitee / Coding / 自建 GitLab 等）
    Domestic,
    /// 国外（GitHub / GitLab.com / Bitbucket 等）
    Overseas,
}

impl Default for GitRegion {
    fn default() -> Self {
        Self::Overseas
    }
}

/// A named git remote with region tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitRemote {
    /// Remote name in git (origin / gitee / github / …).
    pub name: String,
    /// Remote URL (https or ssh).
    pub url: String,
    /// 国内 / 国外 标签，用于切换与展示。
    pub region: GitRegion,
    /// 可选备注（展示用）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Default for GitRemote {
    fn default() -> Self {
        Self {
            name: "origin".into(),
            url: String::new(),
            region: GitRegion::default(),
            note: None,
        }
    }
}

/// Git identity (user.name / user.email). Optional; never force-overwrite global.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl Default for GitIdentity {
    fn default() -> Self {
        Self {
            name: None,
            email: None,
        }
    }
}

/// Auto-commit policy for the host-level `cco git commit` / run post-tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoCommitPolicy {
    /// Master switch. Default false — host never auto-commits unless opted in.
    pub enabled: bool,
    /// Commit message template; `{plan}` = plan name, `{run}` = run_id, `{date}` = today.
    pub message_template: String,
    /// Whether to `git push` after a successful commit.
    pub push_after_commit: bool,
    /// Which remote to push to (by name); empty → current upstream / origin.
    pub push_remote: String,
    /// Which branch to push; empty → current branch.
    pub push_branch: String,
    /// 禁止 force-push（与系统任务一致）。
    pub allow_force: bool,
}

impl Default for AutoCommitPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            message_template: "cco: {plan} ({run})".into(),
            push_after_commit: false,
            push_remote: String::new(),
            push_branch: String::new(),
            allow_force: false,
        }
    }
}

/// Top-level `[git]` config section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Named remotes (国内/国外). Empty → use repo's existing remotes.
    pub remotes: Vec<GitRemote>,
    /// Optional identity to set per-repo (never global).
    pub identity: GitIdentity,
    /// Host-level auto-commit policy (default off).
    pub auto_commit: AutoCommitPolicy,
    /// Default region preference when both domestic & overseas remotes exist.
    /// `domestic` | `overseas`. Used by `cco git push` without explicit --remote.
    pub default_region: GitRegion,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            remotes: Vec::new(),
            identity: GitIdentity::default(),
            auto_commit: AutoCommitPolicy::default(),
            default_region: GitRegion::Overseas,
        }
    }
}

impl GitConfig {
    /// Find a remote by name.
    pub fn remote(&self, name: &str) -> Option<&GitRemote> {
        self.remotes.iter().find(|r| r.name == name)
    }

    /// List remotes filtered by region.
    pub fn remotes_by_region(&self, region: &GitRegion) -> Vec<&GitRemote> {
        self.remotes
            .iter()
            .filter(|r| &r.region == region)
            .collect()
    }

    /// Pick a push remote: explicit name → default_region first match → first remote.
    pub fn pick_push_remote(&self, explicit: Option<&str>) -> Option<&GitRemote> {
        if let Some(name) = explicit.filter(|s| !s.trim().is_empty()) {
            return self.remote(name);
        }
        if let Some(r) = self
            .remotes_by_region(&self.default_region)
            .into_iter()
            .next()
        {
            return Some(r);
        }
        self.remotes.first()
    }

    /// Render a commit message from the template.
    pub fn render_message(&self, plan: &str, run: &str, date: &str) -> String {
        self.auto_commit
            .message_template
            .replace("{plan}", plan)
            .replace("{run}", run)
            .replace("{date}", date)
    }
}

/// Normalize a region token from CLI/UI input. Unknown → None.
pub fn normalize_region(raw: &str) -> Option<GitRegion> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "domestic" | "cn" | "china" | "gitee" | "国内" => Some(GitRegion::Domestic),
        "overseas" | "global" | "github" | "gitlab" | "国外" | "海外" => {
            Some(GitRegion::Overseas)
        }
        _ => None,
    }
}

/// Region display label (中文).
pub fn region_label(region: &GitRegion) -> &'static str {
    match region {
        GitRegion::Domestic => "国内",
        GitRegion::Overseas => "国外",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GitConfig {
        GitConfig {
            remotes: vec![
                GitRemote {
                    name: "gitee".into(),
                    url: "https://gitee.com/u/r.git".into(),
                    region: GitRegion::Domestic,
                    note: Some("镜像".into()),
                },
                GitRemote {
                    name: "github".into(),
                    url: "https://github.com/u/r.git".into(),
                    region: GitRegion::Overseas,
                    note: None,
                },
            ],
            identity: GitIdentity::default(),
            auto_commit: AutoCommitPolicy::default(),
            default_region: GitRegion::Domestic,
        }
    }

    #[test]
    fn pick_explicit_wins() {
        let c = sample();
        let r = c.pick_push_remote(Some("github"));
        assert_eq!(r.map(|r| r.name.as_str()), Some("github"));
    }

    #[test]
    fn pick_default_region_when_no_explicit() {
        let c = sample();
        let r = c.pick_push_remote(None);
        assert_eq!(r.map(|r| r.name.as_str()), Some("gitee"));
    }

    #[test]
    fn pick_falls_back_to_first() {
        let mut c = sample();
        c.default_region = GitRegion::Overseas;
        // remove overseas to force fallback
        c.remotes.retain(|r| r.region == GitRegion::Domestic);
        let r = c.pick_push_remote(None);
        assert_eq!(r.map(|r| r.name.as_str()), Some("gitee"));
    }

    #[test]
    fn render_message_substitutes() {
        let c = GitConfig {
            auto_commit: AutoCommitPolicy {
                message_template: "[{date}] {plan} · {run}".into(),
                ..AutoCommitPolicy::default()
            },
            ..GitConfig::default()
        };
        assert_eq!(
            c.render_message("demo", "r1", "2026-07-30"),
            "[2026-07-30] demo · r1"
        );
    }

    #[test]
    fn normalize_region_aliases() {
        assert_eq!(normalize_region("CN"), Some(GitRegion::Domestic));
        assert_eq!(normalize_region("国内"), Some(GitRegion::Domestic));
        assert_eq!(normalize_region("github"), Some(GitRegion::Overseas));
        assert_eq!(normalize_region("海外"), Some(GitRegion::Overseas));
        assert_eq!(normalize_region("xyz"), None);
    }

    #[test]
    fn region_label_cn() {
        assert_eq!(region_label(&GitRegion::Domestic), "国内");
        assert_eq!(region_label(&GitRegion::Overseas), "国外");
    }

    #[test]
    fn default_off() {
        let c = GitConfig::default();
        assert!(!c.auto_commit.enabled);
        assert!(!c.auto_commit.push_after_commit);
        assert!(!c.auto_commit.allow_force);
        assert!(c.remotes.is_empty());
    }

    #[test]
    fn toml_roundtrip() {
        let c = sample();
        let s = toml::to_string(&c).unwrap();
        let back: GitConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.remotes.len(), 2);
        assert_eq!(back.remotes[0].name, "gitee");
        assert_eq!(back.default_region, GitRegion::Domestic);
    }
}
