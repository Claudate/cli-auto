//! System terminal launchers (macOS Terminal/iTerm, kitty, wezterm, …).
//!
//! [INPUT]: 命令行 + cwd
//! [OUTPUT]: detect_launcher · spawn external window
//! [POS]: TerminalManager external 路径
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/terminal/CLAUDE.md

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLauncher {
    Kitty,
    WezTerm,
    Ghostty,
    ITerm,
    AppleTerminal,
    Tmux,
    XdgTerminal,
    Custom,
}

impl ExternalLauncher {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::WezTerm => "wezterm",
            Self::Ghostty => "ghostty",
            Self::ITerm => "iterm",
            Self::AppleTerminal => "terminal_app",
            Self::Tmux => "tmux",
            Self::XdgTerminal => "xdg",
            Self::Custom => "custom",
        }
    }
}

pub fn detect_launcher(prefer: &str) -> ExternalLauncher {
    let p = prefer.to_ascii_lowercase();
    if p != "auto" && !p.is_empty() {
        return match p.as_str() {
            "kitty" => ExternalLauncher::Kitty,
            "wezterm" => ExternalLauncher::WezTerm,
            "ghostty" => ExternalLauncher::Ghostty,
            "iterm" | "iterm2" => ExternalLauncher::ITerm,
            "terminal_app" | "terminal" | "apple" => ExternalLauncher::AppleTerminal,
            "tmux" => ExternalLauncher::Tmux,
            "custom" => ExternalLauncher::Custom,
            "xdg" => ExternalLauncher::XdgTerminal,
            _ => ExternalLauncher::AppleTerminal,
        };
    }

    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        let t = term.to_ascii_lowercase();
        if t.contains("iterm") {
            return ExternalLauncher::ITerm;
        }
        if t.contains("wezterm") {
            return ExternalLauncher::WezTerm;
        }
        if t.contains("ghostty") {
            return ExternalLauncher::Ghostty;
        }
        if t.contains("apple_terminal") || t.contains("terminal") {
            return ExternalLauncher::AppleTerminal;
        }
        if t.contains("tmux") {
            return ExternalLauncher::Tmux;
        }
    }

    // PATH probes
    if which::which("kitty").is_ok() {
        return ExternalLauncher::Kitty;
    }
    if which::which("wezterm").is_ok() {
        return ExternalLauncher::WezTerm;
    }
    if which::which("ghostty").is_ok() {
        return ExternalLauncher::Ghostty;
    }
    if cfg!(target_os = "macos") {
        if Path::new("/Applications/iTerm.app").exists()
            || Path::new("/Applications/iTerm2.app").exists()
        {
            return ExternalLauncher::ITerm;
        }
        return ExternalLauncher::AppleTerminal;
    }
    if which::which("tmux").is_ok() {
        return ExternalLauncher::Tmux;
    }
    ExternalLauncher::XdgTerminal
}

/// Open an external terminal running `shell_cmd` with cwd.
///
/// `shell_cmd` is a single string executed via `sh -c`.
pub fn open_window(
    launcher: ExternalLauncher,
    cwd: &Path,
    shell_cmd: &str,
    custom_template: Option<&str>,
    title: &str,
) -> Result<Option<u32>> {
    let cwd_s = cwd.display().to_string();
    info!(
        launcher = launcher.as_str(),
        cwd = %cwd_s,
        title,
        "opening external terminal"
    );

    match launcher {
        ExternalLauncher::Kitty => {
            let status = Command::new("kitty")
                .args(["-d", &cwd_s, "-e", "sh", "-c", shell_cmd])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn kitty")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::WezTerm => {
            let status = Command::new("wezterm")
                .args([
                    "start",
                    "--cwd",
                    &cwd_s,
                    "--",
                    "sh",
                    "-c",
                    shell_cmd,
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn wezterm")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::Ghostty => {
            // ghostty -e runs command; cwd via env
            let status = Command::new("ghostty")
                .args(["-e", "sh", "-c", &format!("cd {} && {}", shell_escape(&cwd_s), shell_cmd)])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn ghostty")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::ITerm => {
            // osascript for iTerm2
            let script = format!(
                r#"tell application "iTerm"
  create window with default profile
  tell current session of current window
    write text "cd {cwd} && {cmd}"
  end tell
end tell"#,
                cwd = apple_escape(&cwd_s),
                cmd = apple_escape(shell_cmd),
            );
            let status = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn osascript iTerm")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::AppleTerminal => {
            let script = format!(
                r#"tell application "Terminal"
  do script "cd {cwd} && {cmd}"
  activate
end tell"#,
                cwd = apple_escape(&cwd_s),
                cmd = apple_escape(shell_cmd),
            );
            let status = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn osascript Terminal")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::Tmux => {
            let session = format!("cco-{}", title.replace(' ', "-"));
            let _ = Command::new("tmux")
                .args(["new-session", "-d", "-s", &session, "-c", &cwd_s, shell_cmd])
                .status();
            // also try attach hint — user can tmux attach -t session
            println!("tmux session created: {session}  (tmux attach -t {session})");
            Ok(None)
        }
        ExternalLauncher::XdgTerminal => {
            // common free-desktop
            for bin in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
                if which::which(bin).is_ok() {
                    let child = if bin == "gnome-terminal" {
                        Command::new(bin)
                            .args(["--working-directory", &cwd_s, "--", "sh", "-c", shell_cmd])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                    } else if bin == "konsole" {
                        Command::new(bin)
                            .args(["--workdir", &cwd_s, "-e", "sh", "-c", shell_cmd])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                    } else {
                        Command::new(bin)
                            .args([
                                "-e",
                                "sh",
                                "-c",
                                &format!("cd {} && {}", shell_escape(&cwd_s), shell_cmd),
                            ])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                    };
                    return Ok(child.ok().map(|c| c.id()));
                }
            }
            bail!("no external terminal found on PATH");
        }
        ExternalLauncher::Custom => {
            let tpl = custom_template.ok_or_else(|| {
                anyhow::anyhow!("custom launcher requires terminal.external_command template")
            })?;
            let cmd = tpl
                .replace("{cwd}", &cwd_s)
                .replace("{cmd}", shell_cmd)
                .replace("{title}", title);
            let child = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn custom external terminal")?;
            Ok(Some(child.id()))
        }
    }
}

fn apple_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a default follow-logs command for a task.
pub fn follow_logs_command(stdout_path: &Path, stderr_path: &Path) -> String {
    let out = stdout_path.display();
    let err = stderr_path.display();
    format!(
        "echo 'cco logs · {out}'; \
         (test -f '{out}' && tail -n +1 -f '{out}' || true) & \
         (test -f '{err}' && tail -n +1 -f '{err}' || true) & \
         wait"
    )
}

/// Interactive shell in work dir.
pub fn shell_in_dir_command() -> String {
    "exec ${SHELL:-/bin/zsh} -l".into()
}
