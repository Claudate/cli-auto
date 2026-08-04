//! Global config: ~/.cco/config.toml + env overrides.
//!
//! [INPUT]: 磁盘 config · 环境变量
//! [OUTPUT]: Config · AllowedProject · BrowserConfig · GitConfig · auto_commit_granularity · load/save · runs_dir · failover · post_* · browser
//! [POS]: 全局配置真源；桌面项目白名单存此
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/config/CLAUDE.md

mod git;
pub use git::{
    normalize_auto_commit_granularity, normalize_region, region_label, AutoCommitGranularity,
    AutoCommitPolicy, GitConfig, GitIdentity, GitRegion, GitRemote,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const APP_DIR_NAME: &str = ".cco";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default: DefaultSection,
    pub providers: HashMap<String, ProviderConfig>,
    pub terminal: TerminalConfig,
    pub tui: TuiConfig,
    /// Browser MCP for workers (screenshot / scrape / smoke). Default **off**.
    #[serde(default)]
    pub browser: BrowserConfig,
    /// Git remotes / identity / auto-commit policy. Default off.
    #[serde(default)]
    pub git: GitConfig,
    /// Allowed projects shown in the desktop sidebar (not a filesystem tree).
    #[serde(default)]
    pub projects: Vec<AllowedProject>,
    /// Resolved home state root (~/.cco)
    #[serde(skip)]
    pub state_root: PathBuf,
}

/// Optional browser automation via MCP (Kitewright default · Playwright fallback).
///
/// See `docs/browser-automation-cco.md`. Does **not** embed a browser engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserConfig {
    /// Master switch. Default false — workers get no browser MCP until opted in.
    pub enabled: bool,
    /// `kitewright` (default, chromiumoxide/CDP) | `playwright_mcp`
    pub engine: String,
    /// Launcher binary (`npx`, `kite`, …).
    pub command: String,
    /// Args after command (stdio MCP server).
    pub args: Vec<String>,
    /// Evidence root relative to project (task subdir appended at runtime).
    pub out_dir: String,
    /// ui-verify: prefer injecting preview URL; document gap when missing.
    pub require_preview: bool,
    /// Pass `--strict-mcp-config` so only the task MCP file is used.
    pub strict_mcp: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: "kitewright".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@kitewright/mcp".into()],
            out_dir: ".cco-out/browser".into(),
            require_preview: true,
            strict_mcp: true,
        }
    }
}

impl Config {
    /// Effective host auto-commit granularity.
    ///
    /// Legacy `default.post_git_push_enabled=true` still means per-plan
    /// auto-commit so old configs keep their behavior.
    pub fn auto_commit_granularity(&self) -> AutoCommitGranularity {
        if self.git.auto_commit.enabled
            && self.git.auto_commit.granularity != AutoCommitGranularity::Off
        {
            self.git.auto_commit.granularity
        } else if self.default.post_git_push_enabled {
            AutoCommitGranularity::PerPlan
        } else if self.git.auto_commit.enabled {
            // Previous `[git.auto_commit]` only had `enabled`; preserve the
            // historical opt-in as a plan-level host commit.
            AutoCommitGranularity::PerPlan
        } else {
            AutoCommitGranularity::Off
        }
    }
}

impl BrowserConfig {
    /// Effective enabled: config or `CCO_BROWSER_ENABLED=1|true|yes`.
    pub fn is_enabled(&self) -> bool {
        if self.enabled {
            return true;
        }
        matches!(
            std::env::var("CCO_BROWSER_ENABLED")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        )
    }

    /// Engine id after env override `CCO_BROWSER_ENGINE`.
    pub fn effective_engine(&self) -> String {
        std::env::var("CCO_BROWSER_ENGINE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.engine.clone())
    }

    /// Normalize UI/config engine token → `kitewright` | `playwright_mcp`.
    pub fn normalize_engine(raw: &str) -> Option<String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "kitewright" | "kite" | "chromiumoxide" => Some("kitewright".into()),
            "playwright" | "playwright_mcp" | "playwright-mcp" | "pw" => {
                Some("playwright_mcp".into())
            }
            _ => None,
        }
    }

    /// Apply engine choice and keep `command`/`args` aligned with defaults when
    /// still on the previous engine's stock launcher.
    pub fn apply_engine(&mut self, engine: &str) {
        let Some(norm) = Self::normalize_engine(engine) else {
            return;
        };
        self.engine = norm.clone();
        match norm.as_str() {
            "playwright_mcp" => {
                let on_kite_stock =
                    self.command == "npx" && self.args.iter().any(|a| a.contains("kitewright"));
                if on_kite_stock || self.args.is_empty() {
                    self.command = "npx".into();
                    self.args = vec!["-y".into(), "@playwright/mcp".into()];
                }
            }
            _ => {
                let on_pw_stock =
                    self.command == "npx" && self.args.iter().any(|a| a.contains("playwright"));
                if on_pw_stock || self.args.is_empty() {
                    self.command = "npx".into();
                    self.args = vec!["-y".into(), "@kitewright/mcp".into()];
                }
            }
        }
    }
}

/// A project the user has explicitly allowed / pinned for the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedProject {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub added_at: String,
    /// Persistent plan chosen as default for this project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_plan: Option<PathBuf>,
    /// Plan last used to start a run (convenience shortcut).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_plan: Option<PathBuf>,
}

impl AllowedProject {
    pub fn display_name(&self) -> String {
        if let Some(n) = &self.name {
            if !n.trim().is_empty() {
                return n.clone();
            }
        }
        self.path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| self.path.to_str().unwrap_or("project"))
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultSection {
    pub max_parallel: usize,
    pub poll_interval_secs: u64,
    pub default_mode: String,
    pub default_provider: String,
    pub worktree: bool,
    pub mirror_state: bool,
    pub max_turns: u32,
    /// Per-task default budget (provider opts).
    pub max_budget_usd: f64,
    /// Optional run-level total spend cap across all tasks.
    pub run_max_budget_usd: Option<f64>,
    pub permission_mode: String,
    pub allowed_tools: Vec<String>,
    /// Auto-retries after fail / timeout / stall (in addition to the first try).
    /// 0 = never retry. Effective policy is max(plan.retry_max, this). Cap 10.
    /// UI 文案：「同 CLI 最多再试几次」。
    #[serde(default = "default_retry_max")]
    pub retry_max: u32,
    /// No stdout growth for this many seconds → treat as stall, stop + retry.
    /// Floor 30s (UI clamp), default 180 (3 min; was 600 — too dull for UX).
    /// UI 文案：「多久没新日志算卡死」。User-overridden config.toml is never rewritten.
    #[serde(default = "default_stall_secs")]
    pub stall_secs: u64,
    /// After same-provider retries exhaust, walk [`failover_order`] and retry (H4).
    /// Default true; set false to disable.
    #[serde(default = "default_failover_enabled")]
    pub failover_enabled: bool,
    /// Extra attempts allowed on the fallback provider after a switch (default 1).
    /// Same semantics as retry_max: 1 ⇒ first try + 1 re-try on the new CLI.
    #[serde(default = "default_fallback_extra_attempts")]
    pub fallback_extra_attempts: u32,
    /// Production failover walk order (default claude, codex). Empty → same default.
    /// fake/sdk are never chosen even if listed.
    #[serde(default = "default_failover_order")]
    pub failover_order: Vec<String>,
    /// When true, each Mode B split appends a system optional task「任务巡检」
    /// (not produced by the planner). Confirm screen defaults it **checked**.
    /// Master switch off → never inject. Default **false**.
    #[serde(default = "default_post_feature_off")]
    pub post_inspect_enabled: bool,
    /// When true, each Mode B split appends a system optional task「代码提交 Push」
    /// after business (+ inspect if present). Confirm defaults **checked**.
    /// Master switch off → never inject. Default **false**.
    #[serde(default = "default_post_feature_off")]
    pub post_git_push_enabled: bool,
    /// When true, each Mode B split appends a system optional task「自动开 PR」
    /// after push (or business if push off). Uses local `gh pr create`.
    /// Confirm defaults **checked**. Master switch off → never inject. Default **false**.
    /// Security: runs only when user opts in; never force-push; requires authenticated `gh`.
    #[serde(default = "default_post_feature_off")]
    pub post_open_pr_enabled: bool,
    /// Optional second-pass LLM critic after rule critic (drop bad edges + notes).
    /// Also enabled when env `CCO_PLANNER_CRITIC=1`. Default **false** (cost/latency).
    #[serde(default = "default_post_feature_off")]
    pub planner_critic_enabled: bool,
    /// Claude CLI reasoning effort: `low` | `medium` | `high` | `xhigh` | `max` | `ultracode`.
    /// `ultracode` = xhigh + multi-agent thoroughness hint. Default `high`.
    /// Passed as `claude --effort <level>` (ultracode → xhigh on the flag).
    #[serde(default = "default_effort")]
    pub effort: String,
    /// Ensure: inject `sys-closeout` before inspect when a gate exists (default **true**).
    #[serde(default = "default_ensure_on")]
    pub auto_closeout: bool,
    /// Ensure: after inspect FAIL, auto-start rework when conditions match (default **true**).
    #[serde(default = "default_ensure_on")]
    pub auto_rework: bool,
    /// Ensure: only auto-rework when all blocking ISSUES are docs-closeout.
    /// Default **false** — any real blocking should spawn a rework wave to close
    /// the plan (handwalk residual is demoted host-side and does not trigger).
    #[serde(default = "default_ensure_off")]
    pub auto_rework_docs_only: bool,
    /// P0: role→tier→cheapest available CLI on still-default tasks.
    /// Default **true**. Explicit / tag / force routes are never rewritten.
    /// See `docs/cost-aware-cli-router-2026-07-27.md`.
    #[serde(default = "default_cost_route_on")]
    pub cost_route_enabled: bool,
    /// P1: after same-CLI retries exhaust, prefer a higher-cost tier before
    /// walking [`failover_order`]. Default **true**.
    #[serde(default = "default_cost_route_on")]
    pub cost_escalate_enabled: bool,
    /// P3: heuristic intent (title/prompt/tags) nudges tier. Default **false**
    /// (opt-in; rules stay explainable). No external model proxy.
    #[serde(default = "default_ensure_off")]
    pub cost_intent_enabled: bool,
}

fn default_retry_max() -> u32 {
    2
}
fn default_stall_secs() -> u64 {
    180
}
fn default_failover_enabled() -> bool {
    true
}
fn default_fallback_extra_attempts() -> u32 {
    1
}
fn default_failover_order() -> Vec<String> {
    vec!["claude".into(), "codex".into()]
}
fn default_post_feature_off() -> bool {
    false
}
fn default_effort() -> String {
    "high".into()
}
fn default_ensure_on() -> bool {
    true
}
fn default_ensure_off() -> bool {
    false
}
fn default_cost_route_on() -> bool {
    true
}

/// Allowed effort tokens (product + Claude CLI).
pub const EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultracode"];

/// Normalize user/config effort token. Unknown → None.
pub fn normalize_effort(raw: &str) -> Option<String> {
    let s = raw.trim().to_ascii_lowercase();
    if EFFORT_LEVELS.contains(&s.as_str()) {
        Some(s)
    } else {
        None
    }
}

/// Claude CLI permission modes for unattended workers.
pub const PERMISSION_MODES: &[&str] = &["bypassPermissions", "dontAsk", "acceptEdits", "default"];

/// Normalize permission_mode for Claude worker spawn. Unknown → None.
///
/// Accepts common aliases: `bypass` / `auto` → bypassPermissions;
/// `dont-ask` / `dont_ask` → dontAsk.
pub fn normalize_permission_mode(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Preserve CLI camelCase tokens.
    match s {
        "bypassPermissions" | "dontAsk" | "acceptEdits" | "default" => Some(s.into()),
        _ => match s.to_ascii_lowercase().as_str() {
            "bypass" | "auto" | "bypasspermissions" | "bypass_permissions" => {
                Some("bypassPermissions".into())
            }
            "dontask" | "dont-ask" | "dont_ask" | "deny" => Some("dontAsk".into()),
            "acceptedits" | "accept-edits" | "accept_edits" => Some("acceptEdits".into()),
            "default" => Some("default".into()),
            _ => None,
        },
    }
}

/// True when mode auto-denies tools that need confirmation (no permission UI).
/// Unattended implement workers must not use this — writes become false Done.
pub fn permission_mode_blocks_unattended_writes(mode: &str) -> bool {
    matches!(
        normalize_permission_mode(mode).as_deref(),
        Some("dontAsk") | Some("default")
    )
}

/// CLI `--effort` value: `ultracode` maps to `xhigh` (Claude accepts only low…max).
pub fn effort_cli_level(effort: &str) -> &'static str {
    match effort.trim().to_ascii_lowercase().as_str() {
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        "xhigh" | "ultracode" => "xhigh",
        "max" => "max",
        _ => "high",
    }
}

/// Whether product effort is ultracode (multi-agent thoroughness).
pub fn effort_is_ultracode(effort: &str) -> bool {
    effort.trim().eq_ignore_ascii_case("ultracode")
}

/// Extra system-prompt fragment when ultracode is on (workers + chat).
pub const ULTRACODE_SYSTEM_HINT: &str = "\
Ultracode is on for this turn: optimize for the most exhaustive, correct answer — \
not the fastest or cheapest. Prefer multi-agent style decomposition (find → verify \
→ synthesize), adversarial checks on claims, and thorough coverage. Token cost is \
not a constraint; do not skip verification for speed.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub bin: String,
    pub extra_args: Vec<String>,
    /// Optional per-provider parallel cap (in addition to global max_parallel).
    pub max_parallel: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub default_kind: String,
    pub external_launcher: String,
    /// Template for custom launcher: placeholders {cwd} {cmd} {title}
    pub external_command: Option<String>,
    pub max_embedded: usize,
    pub max_external: usize,
    pub auto_open_on_start: bool,
    pub auto_close_on_done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    pub tick_ms: u64,
    pub default_page: String,
}

impl Default for DefaultSection {
    fn default() -> Self {
        Self {
            max_parallel: 2,
            poll_interval_secs: 5,
            default_mode: "print".into(),
            default_provider: "claude".into(),
            worktree: true,
            mirror_state: false,
            max_turns: 40,
            max_budget_usd: 10.0,
            run_max_budget_usd: None,
            // Unattended workers (print/bg) have no permission UI — dontAsk auto-denies
            // Edit/Bash and yields false Done. Default must auto-authorize writes.
            permission_mode: "bypassPermissions".into(),
            allowed_tools: vec![
                "Read".into(),
                "Edit".into(),
                "Bash".into(),
                "Glob".into(),
                "Grep".into(),
                "Write".into(),
            ],
            retry_max: default_retry_max(),
            stall_secs: default_stall_secs(),
            failover_enabled: default_failover_enabled(),
            fallback_extra_attempts: default_fallback_extra_attempts(),
            failover_order: default_failover_order(),
            post_inspect_enabled: default_post_feature_off(),
            post_git_push_enabled: default_post_feature_off(),
            post_open_pr_enabled: default_post_feature_off(),
            planner_critic_enabled: default_post_feature_off(),
            effort: default_effort(),
            auto_closeout: default_ensure_on(),
            auto_rework: default_ensure_on(),
            auto_rework_docs_only: default_ensure_off(),
            cost_route_enabled: default_cost_route_on(),
            cost_escalate_enabled: default_cost_route_on(),
            cost_intent_enabled: default_ensure_off(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bin: "claude".into(),
            extra_args: vec![],
            max_parallel: None,
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_kind: "embedded".into(),
            external_launcher: "auto".into(),
            external_command: None,
            max_embedded: 6,
            max_external: 8,
            auto_open_on_start: false,
            auto_close_on_done: false,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            tick_ms: 200,
            default_page: "dashboard".into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default: DefaultSection::default(),
            providers: builtin_provider_map(),
            terminal: TerminalConfig::default(),
            tui: TuiConfig::default(),
            browser: BrowserConfig::default(),
            projects: Vec::new(),
            git: GitConfig::default(),
            state_root: default_state_root(),
        }
    }
}

fn provider_cfg(bin: &str, enabled: bool) -> ProviderConfig {
    ProviderConfig {
        enabled,
        bin: bin.into(),
        extra_args: vec![],
        max_parallel: None,
    }
}

/// Built-in provider table (claude/codex/fake/sdk + multi-CLI shell-print).
fn builtin_provider_map() -> HashMap<String, ProviderConfig> {
    let mut providers = HashMap::new();
    providers.insert("claude".into(), provider_cfg("claude", true));
    providers.insert("fake".into(), provider_cfg("fake-claude", true));
    providers.insert("codex".into(), provider_cfg("codex", true));
    // P2-7 non-CLI path: present but off until explicitly enabled.
    providers.insert("sdk".into(), provider_cfg("inline", false));
    // Multi-CLI shell-print (enabled like codex; missing bin → doctor hint).
    for (name, bin) in [
        ("gemini", "gemini"),
        ("qwen", "qwen"),
        ("kimi", "kimi"),
        // DeepSeek channel uses CodeWhale CLI (https://github.com/Hmbown/CodeWhale)
        ("deepseek", "codewhale"),
        ("copilot", "copilot"),
        ("codebuddy", "codebuddy"),
    ] {
        providers.insert(name.into(), provider_cfg(bin, true));
    }
    providers
}

/// Ensure built-in keys exist on load without flipping user enabled flags when present.
fn ensure_builtin_providers(c: &mut Config) {
    for (name, cfg) in builtin_provider_map() {
        c.providers.entry(name).or_insert(cfg);
    }
    // Legacy: deepseek channel used bin "deepseek" / deepseek-tui; product is CodeWhale.
    // Only rewrite bare historical defaults so intentional custom paths stay.
    if let Some(pc) = c.providers.get_mut("deepseek") {
        let b = pc.bin.trim();
        if b == "deepseek" || b == "deepseek-tui" || b.is_empty() {
            pc.bin = "codewhale".into();
        }
    }
}

pub fn default_state_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

impl Config {
    pub fn config_path() -> PathBuf {
        default_state_root().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let mut cfg = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read config {}", path.display()))?;
            let mut c: Config = toml::from_str(&text)
                .with_context(|| format!("parse config {}", path.display()))?;
            c.state_root = default_state_root();
            ensure_builtin_providers(&mut c);
            c
        } else {
            Config::default()
        };

        // Env overrides (shell-print + claude)
        for (env_key, provider) in [
            ("CCO_CLAUDE_BIN", "claude"),
            ("CCO_CODEX_BIN", "codex"),
            ("CCO_GEMINI_BIN", "gemini"),
            ("CCO_QWEN_BIN", "qwen"),
            ("CCO_KIMI_BIN", "kimi"),
            ("CCO_DEEPSEEK_BIN", "codewhale"),
            ("CCO_COPILOT_BIN", "copilot"),
            ("CCO_CODEBUDDY_BIN", "codebuddy"),
        ] {
            if let Ok(bin) = std::env::var(env_key) {
                if !bin.trim().is_empty() {
                    cfg.providers.entry(provider.into()).or_default().bin = bin.clone();
                    if provider == "claude" && bin.contains("fake") {
                        cfg.providers.entry("fake".into()).or_default().bin = bin;
                    }
                }
            }
        }
        if let Ok(p) = std::env::var("CCO_STATE_ROOT") {
            cfg.state_root = PathBuf::from(p);
        }
        if let Ok(p) = std::env::var("CCO_DEFAULT_PROVIDER") {
            cfg.default.default_provider = p;
        }
        if let Ok(e) = std::env::var("CCO_EFFORT") {
            if let Some(n) = normalize_effort(&e) {
                cfg.default.effort = n;
            }
        }

        Ok(cfg)
    }

    pub fn write_template(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let template = r#"# cco global config
# Docs: see claude-cli-orchestrator-plan.md

[default]
max_parallel = 2
poll_interval_secs = 5
default_mode = "print"
default_provider = "claude"
worktree = true
mirror_state = false
max_turns = 40
max_budget_usd = 10.0
# run_max_budget_usd = 25.0
# 同 CLI 最多再试几次（不含首次；0 = 不重试）。
retry_max = 2
# 多久没新日志算卡死（秒）→ stop + 重试（用尽则 pause）。默认 180；旧默认 600 偏钝。
stall_secs = 180
# After same-CLI retries exhaust, walk failover_order (H4).
failover_enabled = true
# Extra attempts allowed on each fallback CLI after a switch (default 1).
fallback_extra_attempts = 1
# Production failover walk order (fake/sdk never chosen). Expand as needed:
# failover_order = ["claude", "codex", "gemini", "qwen", "kimi", "deepseek", "copilot", "codebuddy"]
failover_order = ["claude", "codex"]
# System post-tasks (not from planner): optional tail after every split.
# Off by default; when on, injected as optional + default-checked on confirm.
post_inspect_enabled = false
post_git_push_enabled = false
# Auto-open GitHub PR via local `gh` after push (S-PR). Default off.
# Requires authenticated `gh`; never force-push; confirm-screen optional.
post_open_pr_enabled = false
# Optional second-pass LLM critic after rule critic (also CCO_PLANNER_CRITIC=1).
planner_critic_enabled = false
# Claude reasoning effort: low | medium | high | xhigh | max | ultracode
# ultracode = xhigh + multi-agent thoroughness. Also CCO_EFFORT env.
effort = "high"
# Worker print/bg has no permission UI. Use bypassPermissions so Edit/Bash run.
# dontAsk auto-denies writes and produces false Done (whole plan looks "broken").
permission_mode = "bypassPermissions"
allowed_tools = ["Read", "Edit", "Bash", "Glob", "Grep", "Write"]
# Cost-aware CLI pick on still-default tasks (role→tier→cheapest installed).
# Explicit / tag / --provider / --force-provider are never rewritten.
cost_route_enabled = true
# After same-CLI retries exhaust, try a higher-cost tier before failover_order.
cost_escalate_enabled = true
# P3: keyword/tag intent nudge (typo→cheap, architecture→flagship). Default off.
# cost_intent_enabled = false

[providers.claude]
enabled = true
bin = "claude"
extra_args = []
# max_parallel = 2

[providers.fake]
enabled = true
bin = "fake-claude"

[providers.codex]
enabled = true
bin = "codex"
extra_args = []

# P2-7: non-CLI WorkerPort. Default off — zero product behavior change.
# Enable only for explicit plan provider: sdk (or soft-fill).
# bin = "inline"     → S0 in-process stub (no network)
# bin = "messages"   → S1 Anthropic Messages HTTP one-shot
# bin = "tools"      → S2 Messages tool loop (cwd-scoped read/list/write)
# optional: extra_args = ["claude-sonnet-4-5"]  # model override (or CCO_SDK_MODEL)
# optional: CCO_SDK_API_KEY / ANTHROPIC_API_KEY, CCO_SDK_BASE_URL, CCO_SDK_MAX_TOKENS
# optional: CCO_SDK_BACKEND=inline|messages|tools · CCO_SDK_MAX_TOOL_ROUNDS (S2)
[providers.sdk]
enabled = false
bin = "inline"

[terminal]
default_kind = "embedded"
# auto | iterm | terminal_app | wt | powershell | cmd | kitty | wezterm | ghostty | tmux | xdg | custom
external_launcher = "auto"
# external_command = "kitty -d {cwd} -e sh -c '{cmd}'"
# Windows custom example: external_command = "wt -d {cwd} -- powershell -NoExit -Command {cmd}"
max_embedded = 6
max_external = 8
auto_open_on_start = false
auto_close_on_done = false

[tui]
tick_ms = 200
default_page = "dashboard"

# Browser automation for workers (screenshot / scrape / smoke). Default off.
# Docs: docs/browser-automation-cco.md · engine kitewright (CDP) or playwright_mcp
[browser]
enabled = false
engine = "kitewright"
command = "npx"
args = ["-y", "@kitewright/mcp"]
out_dir = ".cco-out/browser"
require_preview = true
strict_mcp = true

[git]
default_region = "overseas"

[git.auto_commit]
enabled = false
# off | per_plan | per_task
granularity = "off"
push_after_commit = false
allow_force = false
"#;
        std::fs::write(path, template)
            .with_context(|| format!("write config template {}", path.display()))?;
        Ok(())
    }

    pub fn provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.state_root.join("runs")
    }

    /// Persist current config (keeps providers / projects / defaults).
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Serialize without state_root (skipped). Reconstruct a friendly toml.
        let text = toml::to_string_pretty(self).with_context(|| "serialize config")?;
        std::fs::write(&path, text).with_context(|| format!("write config {}", path.display()))?;
        Ok(())
    }

    pub fn add_project(&mut self, path: PathBuf, name: Option<String>) -> Result<AllowedProject> {
        let canon = if path.exists() {
            path.canonicalize().unwrap_or(path)
        } else {
            path
        };
        if let Some(existing) = self.projects.iter().find(|p| paths_equal(&p.path, &canon)) {
            return Ok(existing.clone());
        }
        let entry = AllowedProject {
            path: canon,
            name,
            added_at: chrono::Utc::now().to_rfc3339(),
            default_plan: None,
            last_plan: None,
        };
        self.projects.push(entry.clone());
        self.save()?;
        Ok(entry)
    }

    pub fn remove_project(&mut self, path: &Path) -> Result<bool> {
        let before = self.projects.len();
        self.projects.retain(|p| !paths_equal(&p.path, path));
        if self.projects.len() != before {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => a.to_string_lossy() == b.to_string_lossy(),
    }
}
