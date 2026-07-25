//! Argv / install profiles for shell-print CLIs (codex + multi-CLI expansion).
//!
//! [INPUT]: provider id string
//! [OUTPUT]: ShellProfile constants
//! [POS]: runtime/provider/shell_print
//! [PROTOCOL]: 新 CLI = 加 profile + registry 一行；spawn **禁止** npm install

/// Where the user prompt lands on the CLI argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPlacement {
    /// `cmd [flags…] <prompt>` (gemini / qwen / kimi one-shot style with -p elsewhere).
    /// When `prompt_flag` is Some, uses `flag value` instead of trailing bare arg.
    FlagOrTrailing,
    /// `cmd <subcommand> [flags…] <prompt>` (codex exec).
    SubcommandThenTrailing,
}

/// Static per-CLI shape for headless print workers.
#[derive(Debug, Clone, Copy)]
pub struct ShellProfile {
    pub name: &'static str,
    pub default_bin: &'static str,
    pub bin_env: &'static str,
    /// One-line install hint for preflight / doctor (never auto-run).
    pub install_hint: &'static str,
    /// Official docs / download page (desktop doctor 「官网下载」).
    pub docs_url: &'static str,
    pub version_args: &'static [&'static str],
    pub alt_version_args: Option<&'static [&'static str]>,
    pub placement: PromptPlacement,
    /// e.g. Some("exec") for codex.
    pub subcommand: Option<&'static str>,
    /// Flag that takes the prompt (e.g. "-p"); None → trailing positional.
    pub prompt_flag: Option<&'static str>,
    /// Auto-approve / yolo flags when `full_auto` (default true).
    pub yolo_args: &'static [&'static str],
    /// JSON-ish progressive output flags when `json` (default true).
    pub json_args: &'static [&'static str],
    /// Model flag name (`-m` or `--model`); value from provider_opts.model.
    pub model_flag: Option<&'static str>,
}

pub const CODEX: ShellProfile = ShellProfile {
    name: "codex",
    default_bin: "codex",
    bin_env: "CCO_CODEX_BIN",
    install_hint: "install OpenAI Codex CLI or set CCO_CODEX_BIN",
    docs_url: "https://github.com/openai/codex",
    version_args: &["--version"],
    alt_version_args: Some(&["version"]),
    placement: PromptPlacement::SubcommandThenTrailing,
    subcommand: Some("exec"),
    prompt_flag: None,
    yolo_args: &["--full-auto"],
    json_args: &["--json"],
    model_flag: Some("--model"),
};

pub const GEMINI: ShellProfile = ShellProfile {
    name: "gemini",
    default_bin: "gemini",
    bin_env: "CCO_GEMINI_BIN",
    install_hint: "npm i -g @google/gemini-cli  (or set CCO_GEMINI_BIN)",
    docs_url: "https://github.com/google-gemini/gemini-cli",
    version_args: &["--version"],
    alt_version_args: None,
    placement: PromptPlacement::FlagOrTrailing,
    subcommand: None,
    prompt_flag: Some("-p"),
    yolo_args: &["-y"],
    json_args: &["-o", "json"],
    model_flag: Some("-m"),
};

pub const QWEN: ShellProfile = ShellProfile {
    name: "qwen",
    default_bin: "qwen",
    bin_env: "CCO_QWEN_BIN",
    install_hint: "npm i -g @qwen-code/qwen-code  (or set CCO_QWEN_BIN)",
    docs_url: "https://github.com/QwenLM/qwen-code",
    version_args: &["--version"],
    alt_version_args: None,
    placement: PromptPlacement::FlagOrTrailing,
    subcommand: None,
    prompt_flag: Some("-p"),
    yolo_args: &["-y"],
    json_args: &["-o", "json"],
    model_flag: Some("-m"),
};

pub const KIMI: ShellProfile = ShellProfile {
    name: "kimi",
    default_bin: "kimi",
    bin_env: "CCO_KIMI_BIN",
    install_hint: "npm i -g @moonshot-ai/kimi-code  (or curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash; set CCO_KIMI_BIN)",
    docs_url: "https://moonshotai.github.io/kimi-code/en/",
    version_args: &["--version"],
    alt_version_args: None,
    placement: PromptPlacement::FlagOrTrailing,
    subcommand: None,
    prompt_flag: Some("-p"),
    // Kimi docs: -p one-shot; yolo/auto flags vary by version — empty default, extra_args ok.
    yolo_args: &[],
    json_args: &[],
    model_flag: Some("-m"),
};

/// DeepSeek channel → [CodeWhale](https://github.com/Hmbown/CodeWhale) CLI.
///
/// Rebrand from deepseek-tui: binary is `codewhale`, headless is
/// `codewhale exec --auto [--output-format stream-json] "<prompt>"`.
/// Provider id stays `deepseek` for config/UI stability; bin defaults to `codewhale`.
pub const DEEPSEEK: ShellProfile = ShellProfile {
    name: "deepseek",
    default_bin: "codewhale",
    bin_env: "CCO_DEEPSEEK_BIN",
    install_hint: "npm i -g codewhale  (or curl -fsSL https://codewhale.net/install.sh | sh; set CCO_DEEPSEEK_BIN)",
    docs_url: "https://github.com/Hmbown/CodeWhale",
    version_args: &["--version"],
    alt_version_args: Some(&["version"]),
    placement: PromptPlacement::SubcommandThenTrailing,
    subcommand: Some("exec"),
    prompt_flag: None,
    // --auto: tool-backed agent + auto-approvals (plain exec is model-only, no tools)
    yolo_args: &["--auto"],
    json_args: &["--output-format", "stream-json"],
    model_flag: Some("--model"),
};

pub const COPILOT: ShellProfile = ShellProfile {
    name: "copilot",
    default_bin: "copilot",
    bin_env: "CCO_COPILOT_BIN",
    install_hint: "npm i -g @github/copilot  or  brew install copilot-cli  (set CCO_COPILOT_BIN; requires Copilot auth)",
    docs_url: "https://docs.github.com/copilot/concepts/agents/about-copilot-cli",
    version_args: &["--version"],
    alt_version_args: None,
    placement: PromptPlacement::FlagOrTrailing,
    subcommand: None,
    // Prefer -p when present; users may override via extra_args / provider_opts.
    prompt_flag: Some("-p"),
    yolo_args: &["--allow-all-tools"],
    json_args: &[],
    model_flag: Some("--model"),
};

pub const CODEBUDDY: ShellProfile = ShellProfile {
    name: "codebuddy",
    default_bin: "codebuddy",
    bin_env: "CCO_CODEBUDDY_BIN",
    install_hint: "npm i -g @tencent-ai/codebuddy-code  (or set CCO_CODEBUDDY_BIN)",
    docs_url: "https://cnb.cool/codebuddy/codebuddy-code",
    version_args: &["--version"],
    alt_version_args: Some(&["-v"]),
    placement: PromptPlacement::FlagOrTrailing,
    subcommand: None,
    prompt_flag: Some("-p"),
    yolo_args: &["-y"],
    json_args: &["-o", "json"],
    model_flag: Some("-m"),
};

/// All shell-print production profiles (not claude / fake / sdk).
pub const ALL_SHELL_PROFILES: &[ShellProfile] = &[
    CODEX, GEMINI, QWEN, KIMI, DEEPSEEK, COPILOT, CODEBUDDY,
];

pub fn profile_by_name(name: &str) -> Option<ShellProfile> {
    let n = name.trim().to_ascii_lowercase();
    // Aliases: codewhale / codew → deepseek channel (CodeWhale CLI)
    let n = match n.as_str() {
        "codewhale" | "codew" | "deepseek-tui" => "deepseek".to_string(),
        other => other.to_string(),
    };
    ALL_SHELL_PROFILES
        .iter()
        .copied()
        .find(|p| p.name == n)
}

/// Official docs / download URL for a provider id (shell profiles + claude).
pub fn provider_docs_url(name: &str) -> Option<&'static str> {
    let n = name.trim().to_ascii_lowercase();
    if let Some(p) = profile_by_name(&n) {
        return Some(p.docs_url);
    }
    match n.as_str() {
        "claude" => Some("https://docs.anthropic.com/en/docs/claude-code"),
        "sdk" | "claude-sdk" | "claude_sdk" => {
            Some("https://docs.anthropic.com/en/docs/claude-code")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_profiles_unique_names() {
        let mut names: Vec<&str> = ALL_SHELL_PROFILES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL_SHELL_PROFILES.len());
    }

    #[test]
    fn profile_by_name_lookup() {
        assert_eq!(profile_by_name("gemini").map(|p| p.default_bin), Some("gemini"));
        assert_eq!(profile_by_name("CODEX").map(|p| p.name), Some("codex"));
        assert!(profile_by_name("claude").is_none());
    }

    #[test]
    fn deepseek_is_codewhale_exec() {
        let p = profile_by_name("deepseek").expect("deepseek profile");
        assert_eq!(p.default_bin, "codewhale");
        assert_eq!(p.subcommand, Some("exec"));
        assert_eq!(p.yolo_args, &["--auto"]);
        assert_eq!(p.json_args, &["--output-format", "stream-json"]);
        assert_eq!(p.docs_url, "https://github.com/Hmbown/CodeWhale");
        // aliases resolve to same profile
        assert_eq!(
            profile_by_name("codewhale").map(|x| x.default_bin),
            Some("codewhale")
        );
        assert_eq!(profile_by_name("codew").map(|x| x.name), Some("deepseek"));
    }
}
