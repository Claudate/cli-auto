//! Global config: ~/.cco/config.toml + env overrides.
//!
//! [INPUT]: 磁盘 config · 环境变量
//! [OUTPUT]: Config · AllowedProject · load/save · runs_dir · failover · post_inspect/post_git_push/post_open_pr
//! [POS]: 全局配置真源；桌面项目白名单存此
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/config/CLAUDE.md

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
    /// Allowed projects shown in the desktop sidebar (not a filesystem tree).
    #[serde(default)]
    pub projects: Vec<AllowedProject>,
    /// Resolved home state root (~/.cco)
    #[serde(skip)]
    pub state_root: PathBuf,
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
    /// After same-provider retries exhaust, switch to the other production CLI
    /// (claude↔codex) and retry once more (H4). Default true; set false to disable.
    #[serde(default = "default_failover_enabled")]
    pub failover_enabled: bool,
    /// Extra attempts allowed on the fallback provider after a switch (default 1).
    /// Same semantics as retry_max: 1 ⇒ first try + 1 re-try on the new CLI.
    #[serde(default = "default_fallback_extra_attempts")]
    pub fallback_extra_attempts: u32,
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
fn default_post_feature_off() -> bool {
    false
}
fn default_effort() -> String {
    "high".into()
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
            permission_mode: "dontAsk".into(),
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
            post_inspect_enabled: default_post_feature_off(),
            post_git_push_enabled: default_post_feature_off(),
            post_open_pr_enabled: default_post_feature_off(),
            planner_critic_enabled: default_post_feature_off(),
            effort: default_effort(),
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
        let mut providers = HashMap::new();
        providers.insert(
            "claude".into(),
            ProviderConfig {
                enabled: true,
                bin: "claude".into(),
                extra_args: vec![],
                max_parallel: None,
            },
        );
        providers.insert(
            "fake".into(),
            ProviderConfig {
                enabled: true,
                bin: "fake-claude".into(),
                extra_args: vec![],
                max_parallel: None,
            },
        );
        providers.insert(
            "codex".into(),
            ProviderConfig {
                enabled: true,
                bin: "codex".into(),
                extra_args: vec![],
                max_parallel: None,
            },
        );
        // P2-7 S0 non-CLI path: present in defaults but off until explicitly enabled.
        providers.insert(
            "sdk".into(),
            ProviderConfig {
                enabled: false,
                bin: "inline".into(),
                extra_args: vec![],
                max_parallel: None,
            },
        );
        Self {
            default: DefaultSection::default(),
            providers,
            terminal: TerminalConfig::default(),
            tui: TuiConfig::default(),
            projects: Vec::new(),
            state_root: default_state_root(),
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
            // Ensure built-in providers exist even if file omitted them
            c.providers.entry("claude".into()).or_default();
            c.providers.entry("codex".into()).or_insert(ProviderConfig {
                enabled: true,
                bin: "codex".into(),
                extra_args: vec![],
                max_parallel: None,
            });
            c.providers.entry("fake".into()).or_insert(ProviderConfig {
                enabled: true,
                bin: "fake-claude".into(),
                extra_args: vec![],
                max_parallel: None,
            });
            // Opt-in only: do not flip existing installs to enabled.
            c.providers.entry("sdk".into()).or_insert(ProviderConfig {
                enabled: false,
                bin: "inline".into(),
                extra_args: vec![],
                max_parallel: None,
            });
            c
        } else {
            Config::default()
        };

        // Env overrides
        if let Ok(bin) = std::env::var("CCO_CODEX_BIN") {
            cfg.providers.entry("codex".into()).or_default().bin = bin;
        }
        if let Ok(bin) = std::env::var("CCO_CLAUDE_BIN") {
            cfg.providers
                .entry("claude".into())
                .or_default()
                .bin = bin.clone();
            // Convenience: also wire fake when pointing at a stub
            if bin.contains("fake") {
                cfg.providers.entry("fake".into()).or_default().bin = bin;
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
# After same-CLI retries exhaust, switch claude↔codex and try again (H4).
failover_enabled = true
# Extra attempts allowed on the fallback CLI after a switch (default 1).
fallback_extra_attempts = 1
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
permission_mode = "dontAsk"
allowed_tools = ["Read", "Edit", "Bash", "Glob", "Grep", "Write"]

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
        let text = toml::to_string_pretty(self)
            .with_context(|| "serialize config")?;
        std::fs::write(&path, text)
            .with_context(|| format!("write config {}", path.display()))?;
        Ok(())
    }

    pub fn add_project(&mut self, path: PathBuf, name: Option<String>) -> Result<AllowedProject> {
        let canon = if path.exists() {
            path.canonicalize().unwrap_or(path)
        } else {
            path
        };
        if let Some(existing) = self
            .projects
            .iter()
            .find(|p| paths_equal(&p.path, &canon))
        {
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
        self.projects
            .retain(|p| !paths_equal(&p.path, path));
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
