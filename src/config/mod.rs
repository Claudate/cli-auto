//! Global config: ~/.cco/config.toml + env overrides.

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
}

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

[terminal]
default_kind = "embedded"
external_launcher = "auto"
# external_command = "kitty -d {cwd} -e sh -c '{cmd}'"
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
