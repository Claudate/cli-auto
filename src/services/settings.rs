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
    /// H4: after same-CLI retries exhaust, walk failover_order.
    pub failover_enabled: bool,
    /// Extra attempts on the fallback CLI after a provider switch (default 1).
    pub fallback_extra_attempts: u32,
    /// Production failover walk order (e.g. claude,codex,gemini).
    pub failover_order: Vec<String>,
    /// Human note for settings UI (derived from order).
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
    /// Worker 工具权限：bypassPermissions（推荐·自动写）| dontAsk（无 UI 会拒写）| acceptEdits | default。
    pub permission_mode: String,
    /// 设置页只读说明：无人 worker 必须可写，否则任务会假完成。
    pub permission_mode_note: String,
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
    /// Optional full replacement of failover_order (empty ignored).
    pub failover_order: Option<Vec<String>>,
    pub post_inspect_enabled: Option<bool>,
    pub post_git_push_enabled: Option<bool>,
    pub post_open_pr_enabled: Option<bool>,
    pub planner_critic_enabled: Option<bool>,
    /// low | medium | high | xhigh | max | ultracode
    pub effort: Option<String>,
    /// bypassPermissions | dontAsk | acceptEdits | default（及常见别名）
    pub permission_mode: Option<String>,
}

fn failover_order_note(order: &[String]) -> String {
    let list = if order.is_empty() {
        "claude → codex".to_string()
    } else {
        order.join(" → ")
    };
    format!(
        "备用顺序：{list}；fake/sdk 不参与。同 CLI 重试尽后按序换下一家（preflight 失败则跳过）。"
    )
}

pub fn get_settings(config: &Config) -> SettingsView {
    let order = config.default.failover_order.clone();
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
        failover_order: order.clone(),
        failover_order_note: failover_order_note(&order),
        post_inspect_enabled: config.default.post_inspect_enabled,
        post_git_push_enabled: config.default.post_git_push_enabled,
        post_open_pr_enabled: config.default.post_open_pr_enabled,
        post_tasks_note:
            "系统收尾任务不参与 AI 拆解；开启后每次拆分末尾自动追加为「可选」且默认勾选，确认屏可取消。自动开 PR 需本机已安装并登录 GitHub CLI（gh）；禁止 force-push / 自动 merge。"
                .into(),
        planner_critic_enabled: config.default.planner_critic_enabled,
        effort: config.default.effort.clone(),
        permission_mode: config.default.permission_mode.clone(),
        permission_mode_note: permission_mode_note(&config.default.permission_mode),
    }
}

fn permission_mode_note(mode: &str) -> String {
    match crate::config::normalize_permission_mode(mode)
        .unwrap_or_else(|| "bypassPermissions".into())
        .as_str()
    {
        "bypassPermissions" => {
            "任务可自动写文件与执行命令（推荐）。".into()
        }
        "acceptEdits" => {
            "可自动改文件；部分 shell 命令仍可能被拦。".into()
        }
        "dontAsk" | "default" => {
            "当前会拒绝未授权写操作：无人值守时任务易假完成。请在设置「任务授权」改回自动授权，或点「恢复推荐授权」。".into()
        }
        _ => "任务执行需预先授权写文件。".into(),
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
    if let Some(order) = update.failover_order {
        let cleaned: Vec<String> = order
            .into_iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if !cleaned.is_empty() {
            config.default.failover_order = cleaned;
        }
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
    if let Some(p) = update.permission_mode {
        if let Some(n) = crate::config::normalize_permission_mode(&p) {
            config.default.permission_mode = n;
        }
    }
    config.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_lists_order() {
        let n = failover_order_note(&["claude".into(), "gemini".into(), "qwen".into()]);
        assert!(n.contains("claude → gemini → qwen"));
        assert!(n.contains("fake/sdk"));
    }

    #[test]
    fn get_settings_includes_order() {
        let cfg = Config::default();
        let v = get_settings(&cfg);
        assert_eq!(v.failover_order, vec!["claude".to_string(), "codex".to_string()]);
        assert!(v.failover_order_note.contains("claude"));
    }

    #[test]
    fn default_permission_is_auto_and_settable() {
        let mut cfg = Config::default();
        let v = get_settings(&cfg);
        assert_eq!(v.permission_mode, "bypassPermissions");
        assert!(v.permission_mode_note.contains("自动"));

        set_settings(
            &mut cfg,
            SettingsUpdate {
                poll_interval_secs: None,
                default_provider: None,
                default_mode: None,
                max_parallel: None,
                retry_max: None,
                stall_secs: None,
                failover_enabled: None,
                fallback_extra_attempts: None,
                failover_order: None,
                post_inspect_enabled: None,
                post_git_push_enabled: None,
                post_open_pr_enabled: None,
                planner_critic_enabled: None,
                effort: None,
                permission_mode: Some("dontAsk".into()),
            },
        )
        .unwrap();
        assert_eq!(cfg.default.permission_mode, "dontAsk");
        let v2 = get_settings(&cfg);
        assert!(v2.permission_mode_note.contains("假完成") || v2.permission_mode_note.contains("拒绝"));
    }
}
