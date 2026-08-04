//! Desktop/CLI settings subset of Config.
//!
//! [INPUT]: Config · SettingsUpdate
//! [OUTPUT]: get_settings · set_settings · SettingsView（H4 failover · 费用优选 cost_* · 系统收尾 · 自动提交粒度 · planner_critic · browser）
//! [POS]: services 子模块
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{normalize_auto_commit_granularity, AutoCommitGranularity, Config};

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
    /// Git 自动提交粒度：off | per_plan | per_task。
    pub git_auto_commit_granularity: String,
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
    /// 网页自动化总开关（`config.browser.enabled`；也可 env `CCO_BROWSER_ENABLED`）。
    pub browser_enabled: bool,
    /// `kitewright` | `playwright_mcp`
    pub browser_engine: String,
    /// ui-verify 无本机预览时是否启动失败（`config.browser.require_preview`）。
    pub browser_require_preview: bool,
    /// 设置页只读说明：截图/冒烟/抓取；默认关。
    pub browser_note: String,
    /// P0：still-default 任务按 role→tier 选更便宜 CLI。
    pub cost_route_enabled: bool,
    /// P1：失败后先升档再走 failover_order。
    pub cost_escalate_enabled: bool,
    /// P3：标题/正文/标签启发式微调档位（默认关）。
    pub cost_intent_enabled: bool,
    /// 设置页只读：费用优选说明。
    pub cost_route_note: String,
    // ── Git (host-level auto-commit policy) ──
    pub git_auto_commit_enabled: bool,
    pub git_push_after_commit: bool,
    /// domestic | overseas
    pub git_default_region: String,
    pub git_allow_force: bool,
    /// 设置页只读说明：身份只设本仓库；密钥/.env 不自动 add。
    pub git_note: String,
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
    pub git_auto_commit_granularity: Option<String>,
    pub post_open_pr_enabled: Option<bool>,
    pub planner_critic_enabled: Option<bool>,
    /// low | medium | high | xhigh | max | ultracode
    pub effort: Option<String>,
    /// bypassPermissions | dontAsk | acceptEdits | default（及常见别名）
    pub permission_mode: Option<String>,
    /// 网页自动化总开关 → `config.browser.enabled`
    pub browser_enabled: Option<bool>,
    /// `kitewright` | `playwright_mcp`（及别名）
    pub browser_engine: Option<String>,
    /// ui-verify 是否要求本机预览 URL
    pub browser_require_preview: Option<bool>,
    pub cost_route_enabled: Option<bool>,
    pub cost_escalate_enabled: Option<bool>,
    pub cost_intent_enabled: Option<bool>,
    // ── Git (host-level auto-commit policy) ──
    pub git_auto_commit_enabled: Option<bool>,
    pub git_push_after_commit: Option<bool>,
    /// domestic | overseas
    pub git_default_region: Option<String>,
    pub git_allow_force: Option<bool>,
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
        post_git_push_enabled: config.auto_commit_granularity() == AutoCommitGranularity::PerPlan,
        git_auto_commit_granularity: config.auto_commit_granularity().as_str().into(),
        post_open_pr_enabled: config.default.post_open_pr_enabled,
        post_tasks_note:
            "系统收尾任务不参与 AI 拆解；开启后每次拆分末尾自动追加为「可选」且默认勾选，确认屏可取消。自动开 PR 需本机已安装并登录 GitHub CLI（gh）；禁止 force-push / 自动 merge。"
                .into(),
        planner_critic_enabled: config.default.planner_critic_enabled,
        effort: config.default.effort.clone(),
        permission_mode: config.default.permission_mode.clone(),
        permission_mode_note: permission_mode_note(&config.default.permission_mode),
        browser_enabled: config.browser.is_enabled(),
        browser_engine: config.browser.effective_engine(),
        browser_require_preview: config.browser.require_preview,
        browser_note: if config.browser.is_enabled() {
            format!(
                "已开：引擎 {} · 预览门闩 {} · 产物 {} · 需 Chrome + MCP；步骤 tags 含 browser。Claude 可注入 MCP；其他通道请在本机 CLI 自行配置。",
                config.browser.effective_engine(),
                if config.browser.require_preview {
                    "开"
                } else {
                    "关"
                },
                config.browser.out_dir
            )
        } else {
            "默认关。开启后，带 browser 标签的步骤可截图/冒烟/抓取（需 Chrome）。详见 docs/browser-automation-cco.md。"
                .into()
        },
        cost_route_enabled: config.default.cost_route_enabled,
        cost_escalate_enabled: config.default.cost_escalate_enabled,
        cost_intent_enabled: config.default.cost_intent_enabled,
        cost_route_note: cost_route_note(
            config.default.cost_route_enabled,
            config.default.cost_escalate_enabled,
            config.default.cost_intent_enabled,
        ),
        git_auto_commit_enabled: config.auto_commit_granularity() != AutoCommitGranularity::Off,
        git_push_after_commit: config.git.auto_commit.push_after_commit,
        git_default_region: format!("{:?}", config.git.default_region).to_ascii_lowercase(),
        git_allow_force: config.git.auto_commit.allow_force,
        git_note: git_note(config),
    }
}

fn git_note(config: &Config) -> String {
    let granularity = config.auto_commit_granularity();
    if granularity != AutoCommitGranularity::Off {
        let region = crate::config::region_label(&config.git.default_region);
        let push = if config.git.auto_commit.push_after_commit {
            "提交后自动 push"
        } else {
            "仅提交不 push"
        };
        let force = if config.git.auto_commit.allow_force {
            " · 允许 force-push"
        } else {
            ""
        };
        let grain = match granularity {
            AutoCommitGranularity::Off => "关闭",
            AutoCommitGranularity::PerPlan => "按计划提交",
            AutoCommitGranularity::PerTask => "按拆分任务提交",
        };
        format!("已开：{grain} · {push} · 默认区域 {region}{force}。身份只设本仓库；密钥/.env 不自动 add。")
    } else {
        "默认关。可选择按计划提交或按拆分任务提交；身份只设本仓库 --local；密钥/.env 不自动 add；force-push 需显式 --force 且 allow_force。"
            .into()
    }
}

fn cost_route_note(route: bool, escalate: bool, intent: bool) -> String {
    let mut bits = Vec::new();
    if route {
        bits.push("自动选更便宜通道");
    } else {
        bits.push("已关自动选通道");
    }
    if escalate {
        bits.push("失败可升档");
    }
    if intent {
        bits.push("意图微调已开");
    }
    bits.push("你指定的通道不动");
    bits.join(" · ")
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
        if v {
            config.git.auto_commit.enabled = true;
            config.git.auto_commit.granularity = AutoCommitGranularity::PerPlan;
        } else if !config.git.auto_commit.enabled {
            config.git.auto_commit.granularity = AutoCommitGranularity::Off;
        }
    }
    if let Some(raw) = update.git_auto_commit_granularity {
        if let Some(granularity) = normalize_auto_commit_granularity(&raw) {
            config.git.auto_commit.granularity = granularity;
            config.git.auto_commit.enabled = granularity != AutoCommitGranularity::Off;
            // New host-owned modes do not inject the legacy AI system task.
            config.default.post_git_push_enabled = false;
        }
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
    if let Some(v) = update.browser_enabled {
        config.browser.enabled = v;
    }
    if let Some(e) = update.browser_engine {
        config.browser.apply_engine(&e);
    }
    if let Some(v) = update.browser_require_preview {
        config.browser.require_preview = v;
    }
    if let Some(v) = update.cost_route_enabled {
        config.default.cost_route_enabled = v;
    }
    if let Some(v) = update.cost_escalate_enabled {
        config.default.cost_escalate_enabled = v;
    }
    if let Some(v) = update.cost_intent_enabled {
        config.default.cost_intent_enabled = v;
    }
    if let Some(v) = update.git_auto_commit_enabled {
        config.git.auto_commit.enabled = v;
        if !v {
            config.git.auto_commit.granularity = AutoCommitGranularity::Off;
            config.default.post_git_push_enabled = false;
        } else if config.git.auto_commit.granularity == AutoCommitGranularity::Off {
            config.git.auto_commit.granularity = AutoCommitGranularity::PerPlan;
            config.default.post_git_push_enabled = false;
        }
    }
    if let Some(v) = update.git_push_after_commit {
        config.git.auto_commit.push_after_commit = v;
    }
    if let Some(r) = update.git_default_region {
        if let Some(reg) = crate::config::normalize_region(&r) {
            config.git.default_region = reg;
        }
    }
    if let Some(v) = update.git_allow_force {
        config.git.auto_commit.allow_force = v;
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
        assert_eq!(
            v.failover_order,
            vec!["claude".to_string(), "codex".to_string()]
        );
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
                git_auto_commit_granularity: Some("per_task".into()),
                post_open_pr_enabled: None,
                planner_critic_enabled: None,
                effort: None,
                permission_mode: Some("dontAsk".into()),
                browser_enabled: None,
                browser_engine: None,
                browser_require_preview: None,
                cost_route_enabled: None,
                cost_escalate_enabled: None,
                cost_intent_enabled: Some(true),
                git_auto_commit_enabled: None,
                git_push_after_commit: None,
                git_default_region: None,
                git_allow_force: None,
            },
        )
        .unwrap();
        assert_eq!(cfg.default.permission_mode, "dontAsk");
        assert!(cfg.default.cost_intent_enabled);
        assert_eq!(
            cfg.git.auto_commit.granularity,
            AutoCommitGranularity::PerTask
        );
        assert!(cfg.git.auto_commit.enabled);
        assert_eq!(get_settings(&cfg).git_auto_commit_granularity, "per_task");
    }

    #[test]
    fn cost_route_defaults_and_settable() {
        let mut cfg = Config::default();
        let v = get_settings(&cfg);
        assert!(v.cost_route_enabled);
        assert!(v.cost_escalate_enabled);
        assert!(!v.cost_intent_enabled);
        assert!(v.cost_route_note.contains("便宜"));
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
                git_auto_commit_granularity: None,
                post_open_pr_enabled: None,
                planner_critic_enabled: None,
                effort: None,
                permission_mode: None,
                browser_enabled: None,
                browser_engine: None,
                browser_require_preview: None,
                cost_route_enabled: Some(false),
                cost_escalate_enabled: Some(false),
                cost_intent_enabled: Some(true),
                git_auto_commit_enabled: None,
                git_push_after_commit: None,
                git_default_region: None,
                git_allow_force: None,
            },
        )
        .unwrap();
        assert!(!cfg.default.cost_route_enabled);
        assert!(!cfg.default.cost_escalate_enabled);
        assert!(cfg.default.cost_intent_enabled);
        let v2 = get_settings(&cfg);
        assert!(!v2.cost_route_enabled);
        assert!(!v2.cost_escalate_enabled);
        assert!(v2.cost_intent_enabled);
        assert!(
            v2.cost_route_note.contains("已关") || v2.cost_route_note.contains("意图"),
            "note={}",
            v2.cost_route_note
        );
    }

    #[test]
    fn browser_enabled_roundtrip() {
        let mut cfg = Config::default();
        assert!(!get_settings(&cfg).browser_enabled);
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
                git_auto_commit_granularity: None,
                post_open_pr_enabled: None,
                planner_critic_enabled: None,
                effort: None,
                permission_mode: None,
                browser_enabled: Some(true),
                browser_engine: None,
                browser_require_preview: None,
                cost_route_enabled: None,
                cost_escalate_enabled: None,
                cost_intent_enabled: None,
                git_auto_commit_enabled: None,
                git_push_after_commit: None,
                git_default_region: None,
                git_allow_force: None,
            },
        )
        .unwrap();
        assert!(cfg.browser.enabled);
        assert!(get_settings(&cfg).browser_enabled);
        assert!(
            get_settings(&cfg).browser_note.contains("已开")
                || get_settings(&cfg).browser_note.contains("引擎")
        );
    }

    #[test]
    fn browser_engine_and_require_preview_settable() {
        let mut cfg = Config::default();
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
                git_auto_commit_granularity: None,
                post_open_pr_enabled: None,
                planner_critic_enabled: None,
                effort: None,
                permission_mode: None,
                browser_enabled: Some(true),
                browser_engine: Some("playwright".into()),
                browser_require_preview: Some(false),
                cost_route_enabled: None,
                cost_escalate_enabled: None,
                cost_intent_enabled: None,
                git_auto_commit_enabled: None,
                git_push_after_commit: None,
                git_default_region: None,
                git_allow_force: None,
            },
        )
        .unwrap();
        assert_eq!(cfg.browser.engine, "playwright_mcp");
        assert!(!cfg.browser.require_preview);
        assert!(cfg.browser.args.iter().any(|a| a.contains("playwright")));
        let v = get_settings(&cfg);
        assert!(v.browser_engine.contains("playwright"));
        assert!(!v.browser_require_preview);
    }
}
