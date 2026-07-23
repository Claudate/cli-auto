//! Desktop/CLI settings subset of Config.
//!
//! [INPUT]: Config · SettingsUpdate
//! [OUTPUT]: get_settings · set_settings · SettingsView（H4 failover · 系统收尾 post_inspect/post_git_push/post_open_pr · planner_critic_enabled）
//! [POS]: services 子模块
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Subset of config exposed to the desktop UI for reading.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub poll_interval_secs: u64,
    pub default_provider: String,
    pub default_mode: String,
    pub max_parallel: usize,
    pub ui_refresh_secs: u64,
    /// 同 CLI 最多再试几次（不含首次；0–10）。
    pub retry_max: u32,
    /// 多久没新日志算卡死（秒；无日志增长 → stop + 重试）。
    pub stall_secs: u64,
    /// H4: after same-CLI retries exhaust, switch claude↔codex and try again.
    pub failover_enabled: bool,
    /// Extra attempts on the fallback CLI after a provider switch (default 1).
    pub fallback_extra_attempts: u32,
    /// Read-only human note for settings UI (not persisted separately).
    pub failover_order_note: String,
    /// 拆分后附加系统任务「任务巡检」（可选，默认勾选）。总开关默认关。
    pub post_inspect_enabled: bool,
    /// 拆分后附加系统任务「代码提交 Push」（可选，默认勾选）。总开关默认关。
    pub post_git_push_enabled: bool,
    /// 拆分后附加系统任务「自动开 PR」（可选，默认勾选）。总开关默认关。需本机 `gh` 已登录。
    pub post_open_pr_enabled: bool,
    /// 设置页只读说明：系统收尾不参与拆解。
    pub post_tasks_note: String,
    /// 拆分后可选 LLM 第二跳校对（去假依赖）；默认关。也可 env CCO_PLANNER_CRITIC=1。
    pub planner_critic_enabled: bool,
    /// 推理深度：low | medium | high | xhigh | max | ultracode（ultracode=xhigh+多 Agent）。
    pub effort: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsUpdate {
    pub poll_interval_secs: Option<u64>,
    pub default_provider: Option<String>,
    pub default_mode: Option<u32>,
    pub max_parallel: Option<usize>,
    pub retry_max: Option<u32>,
    pub stall_secs: Option<u64>,
    pub failover_enabled: Option<bool>,
    pub fallback_extra_attempts: Option<u32>,
    pub post_inspect_enabled: Option<bool>,
    pub post_git_push_enabled: Option<bool>,
    pub post_open_pr_enabled: Option<bool>,
    pub planner_critic_enabled: Option<bool>,
    /// low | medium | high | xhigh | max | ultracode
    pub effort: Option<String>,
}

pub fn get_settings(config: &Config) -> SettingsView {
    SettingsView {
        poll_interval_secs: config.default.poll_interval_secs,
        default_provider: config.default.default_provider.clone(),
        default_mode: config.default.default_mode.clone(),
        max_parallel: config.default.max_parallel,
        ui_refresh_secs: 2, // UI hardcoded; could become configurable later
        retry_max: config.default.retry_max,
        stall_secs: config.default.stall_secs,
        failover_enabled: config.default.failover_enabled,
        fallback_extra_attempts: config.default.fallback_extra_attempts,
        // 设置页只读说明（H4）：顺序固定，fake 不参与生产 failover
        failover_order_note:
            "备用顺序只读：claude ↔ codex；fake 不参与。同 CLI 重试尽后自动换另一家再试。".into(),
        post_inspect_enabled: config.default.post_inspect_enabled,
        post_git_push_enabled: config.default.post_git_push_enabled,
        post_open_pr_enabled: config.default.post_open_pr_enabled,
        post_tasks_note:
            "系统收尾任务不参与 AI 拆解；开启后每次拆分末尾自动追加为「可选」且默认勾选，确认屏可取消。自动开 PR 需本机已安装并登录 GitHub CLI（gh）；禁止 force-push / 自动 merge。"
                .into(),
        planner_critic_enabled: config.default.planner_critic_enabled,
        effort: config.default.effort.clone(),
    }
}

/// Apply partial update to config and persist.
pub fn set_settings(config: &mut Config, update: SettingsUpdate) -> Result<()> {
    if let Some(v) = update.poll_interval_secs {
        config.default.poll_interval_secs = v.clamp(1, 60);
    }
    if let Some(p) = update.default_provider {
        if !p.is_empty() {
            config.default.default_provider = p;
        }
    }
    if let Some(m) = update.default_mode {
        match m {
            0 => config.default.default_mode = "print".to_string(),
            1 => config.default.default_mode = "bg".to_string(),
            2 => config.default.default_mode = "auto".to_string(),
            _ => {}
        }
    }
    if let Some(v) = update.max_parallel {
        config.default.max_parallel = v.clamp(1, 32);
    }
    if let Some(v) = update.retry_max {
        config.default.retry_max = v.min(10);
    }
    if let Some(v) = update.stall_secs {
        // 60s–2h production range; allow lower only via config.toml for tests.
        config.default.stall_secs = v.clamp(30, 7200);
    }
    if let Some(v) = update.failover_enabled {
        config.default.failover_enabled = v;
    }
    if let Some(v) = update.fallback_extra_attempts {
        config.default.fallback_extra_attempts = v.min(10);
    }
    if let Some(v) = update.post_inspect_enabled {
        config.default.post_inspect_enabled = v;
    }
    if let Some(v) = update.post_git_push_enabled {
        config.default.post_git_push_enabled = v;
    }
    if let Some(v) = update.post_open_pr_enabled {
        config.default.post_open_pr_enabled = v;
    }
    if let Some(v) = update.planner_critic_enabled {
        config.default.planner_critic_enabled = v;
    }
    if let Some(e) = update.effort {
        if let Some(n) = crate::config::normalize_effort(&e) {
            config.default.effort = n;
        }
    }
    config.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_critic_view_and_update_field() {
        let mut cfg = Config::default();
        assert!(!cfg.default.planner_critic_enabled);
        assert!(!get_settings(&cfg).planner_critic_enabled);

        cfg.default.planner_critic_enabled = true;
        assert!(get_settings(&cfg).planner_critic_enabled);

        let u: SettingsUpdate =
            serde_json::from_str(r#"{"planner_critic_enabled":true}"#).unwrap();
        assert_eq!(u.planner_critic_enabled, Some(true));
        // Partial update deserializes missing fields as None (does not force false)
        let u2: SettingsUpdate = serde_json::from_str(r#"{"max_parallel":3}"#).unwrap();
        assert_eq!(u2.planner_critic_enabled, None);
    }

    #[test]
    fn effort_view_and_update_field() {
        let mut cfg = Config::default();
        assert_eq!(cfg.default.effort, "high");
        assert_eq!(get_settings(&cfg).effort, "high");

        let u: SettingsUpdate = serde_json::from_str(r#"{"effort":"ultracode"}"#).unwrap();
        assert_eq!(u.effort.as_deref(), Some("ultracode"));
        // Apply without save (set_settings writes disk); mirror its normalize path.
        if let Some(n) = crate::config::normalize_effort(u.effort.as_deref().unwrap()) {
            cfg.default.effort = n;
        }
        assert_eq!(cfg.default.effort, "ultracode");
        assert_eq!(get_settings(&cfg).effort, "ultracode");

        // Invalid token ignored
        let bad: SettingsUpdate = serde_json::from_str(r#"{"effort":"turbo"}"#).unwrap();
        if let Some(raw) = bad.effort.as_deref() {
            if let Some(n) = crate::config::normalize_effort(raw) {
                cfg.default.effort = n;
            }
        }
        assert_eq!(cfg.default.effort, "ultracode");
        assert_eq!(
            crate::config::effort_cli_level("ultracode"),
            "xhigh"
        );
        assert!(crate::config::effort_is_ultracode("ultracode"));
    }

    #[test]
    fn open_pr_view_and_update_field() {
        let mut cfg = Config::default();
        assert!(!cfg.default.post_open_pr_enabled);
        assert!(!get_settings(&cfg).post_open_pr_enabled);

        let u: SettingsUpdate =
            serde_json::from_str(r#"{"post_open_pr_enabled":true}"#).unwrap();
        assert_eq!(u.post_open_pr_enabled, Some(true));
        // Partial update keeps other fields None
        let u2: SettingsUpdate = serde_json::from_str(r#"{"post_git_push_enabled":true}"#).unwrap();
        assert_eq!(u2.post_open_pr_enabled, None);

        cfg.default.post_open_pr_enabled = true;
        assert!(get_settings(&cfg).post_open_pr_enabled);
        assert!(
            get_settings(&cfg)
                .post_tasks_note
                .contains("gh"),
            "settings note should mention gh for PR"
        );
    }
}
