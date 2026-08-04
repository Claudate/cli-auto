//! Chat CLI selection (L1): available print-capable CLIs + provider dispatch.
//!
//! [INPUT]: Config · cli name (provider id)
//! [OUTPUT]: Box<dyn WorkerPort> chat provider · ChatCliInfo list for the UI dropdown
//! [POS]: services/chat — thin dispatch; shell-print profiles stay in `runtime/provider`
//! [PROTOCOL]: 变更时更新此头部；默认仍 claude（显式覆盖才换 CLI）；不旁路 confirm
//!
//! Chat contract (stdout/.done/scope/env) is shared by claude and shell-print providers,
//! so dispatch is a plain `WorkerPort` swap. `fake` is a template path (send.rs), not
//! a spawnable CLI — dispatch bails on it.

use anyhow::{bail, Result};
use serde::Serialize;

use crate::config::Config;
use crate::runtime::provider::claude::ClaudeProvider;
use crate::runtime::provider::shell_print::{profile_by_name, ShellPrintProvider};
use crate::runtime::provider::{resolve_provider_bin, ProviderRegistry, WorkerPort};

/// Chat-capable CLI info for the UI dropdown.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCliInfo {
    pub name: String,
    pub label: String,
    /// Config-enabled (providers.<name>.enabled).
    pub enabled: bool,
    /// Supports print interaction (chat needs it).
    pub print_capable: bool,
}

/// List CLIs usable from chat: enabled print-capable providers (claude first).
pub fn available_chat_clis(config: &Config) -> Vec<ChatCliInfo> {
    let registry = match ProviderRegistry::from_config(config) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "chat clis: registry build failed");
            return vec![];
        }
    };
    let mut infos: Vec<ChatCliInfo> = registry
        .list()
        .into_iter()
        .map(|name| {
            let print_capable = registry
                .get(name)
                .map(|p| p.capabilities().print)
                .unwrap_or(false);
            ChatCliInfo {
                name: name.to_string(),
                label: cli_label(name).to_string(),
                enabled: config.provider(name).map(|p| p.enabled).unwrap_or(false),
                print_capable,
            }
        })
        .collect();
    // Stable order: fake last, otherwise registry order (claude/codex first).
    infos.sort_by_key(|i| i.name == "fake");
    infos
}

/// Build the chat provider for a CLI name (None → default claude).
pub fn chat_provider(config: &Config, cli: Option<&str>) -> Result<Box<dyn WorkerPort>> {
    let name = cli.unwrap_or("claude");
    match name {
        "fake" => bail!("fake is a template reply path, not a spawnable chat CLI"),
        "claude" => Ok(Box::new(claude_provider(config))),
        other => {
            let profile = profile_by_name(other).ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown chat CLI {other:?} — available: claude, fake, codex, gemini, qwen, kimi, deepseek, copilot, codebuddy"
                )
            })?;
            let cfg = config
                .provider(other)
                .ok_or_else(|| anyhow::anyhow!("provider {other:?} not configured"))?;
            if !cfg.enabled {
                bail!("provider {other:?} disabled in config (providers.{other}.enabled)");
            }
            let bin = resolve_provider_bin(&cfg.bin, profile.bin_env);
            Ok(Box::new(ShellPrintProvider::new(
                profile,
                bin,
                cfg.extra_args.clone(),
            )))
        }
    }
}

/// Provider-opts for a chat turn: claude uses the full option set; shell-print
/// providers only read `timeout_secs` (their CLI flags come from the profile).
pub fn chat_provider_opts(cli: Option<&str>, effort: &str) -> serde_json::Value {
    if cli == Some("fake") || (cli.is_some() && cli != Some("claude")) {
        serde_json::json!({ "timeout_secs": 600 })
    } else {
        serde_json::json!({
            // null = omit CLI limit flags (see ClaudeProvider::opt_limit_*).
            "max_turns": null,
            "max_budget_usd": null,
            // Desktop chat has no permission UI: dontAsk denied Bash (e.g. npm run dev).
            // bypassPermissions + allow flag (spawn) lets install/start/preview run in-project.
            // Scope still locked by --append-system-prompt (project dir only).
            "permission_mode": "bypassPermissions",
            // Reasoning depth → claude --effort (ultracode → xhigh + system hint).
            "effort": effort,
        })
    }
}

pub(crate) fn claude_provider(config: &Config) -> ClaudeProvider {
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    ClaudeProvider::new(bin, extra)
}

/// Product-facing label (same wording as the split-desk channel dropdown).
fn cli_label(name: &str) -> &'static str {
    match name {
        "claude" => "Claude · 默认",
        "codex" => "Codex",
        "fake" => "演练 fake",
        "gemini" => "Gemini",
        "qwen" => "通义 Qwen",
        "kimi" => "Kimi",
        "deepseek" => "CodeWhale",
        "copilot" => "Copilot",
        "codebuddy" => "CodeBuddy",
        "sdk" => "SDK（非 CLI）",
        _ => "自定义 CLI",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_cfg() -> (tempfile::TempDir, Config) {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        (dir, cfg)
    }

    #[test]
    fn default_is_claude() {
        let (_d, cfg) = test_cfg();
        let p = chat_provider(&cfg, None).unwrap();
        assert!(p.capabilities().print);
    }

    #[test]
    fn fake_is_rejected_by_dispatch() {
        let (_d, cfg) = test_cfg();
        assert!(chat_provider(&cfg, Some("fake")).is_err());
    }

    #[test]
    fn unknown_cli_is_rejected() {
        let (_d, cfg) = test_cfg();
        assert!(chat_provider(&cfg, Some("nope")).is_err());
    }

    #[test]
    fn codex_dispatch_works_when_enabled() {
        let (_d, cfg) = test_cfg();
        let p = chat_provider(&cfg, Some("codex")).unwrap();
        assert!(p.capabilities().print);
    }

    #[test]
    fn available_list_contains_claude_and_fake() {
        let (_d, cfg) = test_cfg();
        let list = available_chat_clis(&cfg);
        assert!(list.iter().any(|i| i.name == "claude"));
        assert!(list.iter().any(|i| i.name == "fake"));
        let claude = list.iter().find(|i| i.name == "claude").unwrap();
        assert!(claude.enabled && claude.print_capable);
    }

    #[test]
    fn opts_are_minimal_for_shell_but_full_for_claude() {
        let shell = chat_provider_opts(Some("codex"), "high");
        assert_eq!(shell.get("timeout_secs").and_then(|v| v.as_u64()), Some(600));
        assert!(shell.get("effort").is_none());
        let claude = chat_provider_opts(None, "high");
        assert_eq!(claude.get("effort").and_then(|v| v.as_str()), Some("high"));
    }
}
