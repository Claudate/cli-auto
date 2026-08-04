//! System terminal launchers (macOS Terminal/iTerm, Windows Terminal/cmd, kitty, wezterm, …).
//!
//! [INPUT]: 命令行 + cwd
//! [OUTPUT]: detect_launcher · spawn external window
//! [POS]: TerminalManager external 路径
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/terminal/CLAUDE.md

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use tracing::info;

use super::win;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLauncher {
    Kitty,
    WezTerm,
    Ghostty,
    ITerm,
    AppleTerminal,
    /// Windows Terminal (`wt.exe`) — P2-7 Windows thin slice.
    WindowsTerminal,
    /// Windows `powershell.exe` / `pwsh` new window.
    PowerShell,
    /// Windows `cmd.exe` via `start`.
    Cmd,
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
            Self::WindowsTerminal => "wt",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
            Self::Tmux => "tmux",
            Self::XdgTerminal => "xdg",
            Self::Custom => "custom",
        }
    }
}

/// Platform default when prefer is empty / unknown / auto with no probes.
fn platform_default() -> ExternalLauncher {
    if cfg!(windows) {
        ExternalLauncher::Cmd
    } else if cfg!(target_os = "macos") {
        ExternalLauncher::AppleTerminal
    } else {
        ExternalLauncher::XdgTerminal
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
            "terminal_app" | "apple" => ExternalLauncher::AppleTerminal,
            // bare "terminal": Windows Terminal on Windows, Apple Terminal elsewhere
            "terminal" => {
                if cfg!(windows) {
                    ExternalLauncher::WindowsTerminal
                } else {
                    ExternalLauncher::AppleTerminal
                }
            }
            "wt" | "windows_terminal" | "windowsterminal" | "windows-terminal" => {
                ExternalLauncher::WindowsTerminal
            }
            "powershell" | "pwsh" | "ps" => ExternalLauncher::PowerShell,
            "cmd" | "command_prompt" | "cmd.exe" => ExternalLauncher::Cmd,
            "tmux" => ExternalLauncher::Tmux,
            "custom" => ExternalLauncher::Custom,
            "xdg" => ExternalLauncher::XdgTerminal,
            _ => platform_default(),
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
        if t.contains("apple_terminal") || t == "terminal" {
            return ExternalLauncher::AppleTerminal;
        }
        if t.contains("tmux") {
            return ExternalLauncher::Tmux;
        }
    }
    // Windows Terminal sets WT_SESSION when running inside it.
    if cfg!(windows) && std::env::var_os("WT_SESSION").is_some() {
        return ExternalLauncher::WindowsTerminal;
    }

    // PATH probes (cross-platform first)
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
    if cfg!(windows) {
        if which::which("wt.exe").is_ok() || which::which("wt").is_ok() {
            return ExternalLauncher::WindowsTerminal;
        }
        if which::which("pwsh.exe").is_ok()
            || which::which("pwsh").is_ok()
            || which::which("powershell.exe").is_ok()
            || which::which("powershell").is_ok()
        {
            return ExternalLauncher::PowerShell;
        }
        return ExternalLauncher::Cmd;
    }
    if which::which("tmux").is_ok() {
        return ExternalLauncher::Tmux;
    }
    ExternalLauncher::XdgTerminal
}

/// Open an external terminal running `shell_cmd` with cwd.
///
/// On Unix, `shell_cmd` is executed via `sh -c`.
/// On Windows native launchers, it is executed via PowerShell `-Command` or `cmd /K`.
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
                .args(["start", "--cwd", &cwd_s, "--", "sh", "-c", shell_cmd])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("spawn wezterm")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::Ghostty => {
            let status = Command::new("ghostty")
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
                .context("spawn ghostty")?;
            Ok(Some(status.id()))
        }
        ExternalLauncher::ITerm => {
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
        ExternalLauncher::WindowsTerminal => win::open_windows_terminal(&cwd_s, shell_cmd),
        ExternalLauncher::PowerShell => win::open_powershell_window(&cwd_s, shell_cmd),
        ExternalLauncher::Cmd => win::open_cmd_window(&cwd_s, shell_cmd),
        ExternalLauncher::Tmux => {
            let session = format!("cco-{}", title.replace(' ', "-"));
            let _ = Command::new("tmux")
                .args(["new-session", "-d", "-s", &session, "-c", &cwd_s, shell_cmd])
                .status();
            println!("tmux session created: {session}  (tmux attach -t {session})");
            Ok(None)
        }
        ExternalLauncher::XdgTerminal => open_xdg_terminal(&cwd_s, shell_cmd),
        ExternalLauncher::Custom => {
            let tpl = custom_template.ok_or_else(|| {
                anyhow::anyhow!("custom launcher requires terminal.external_command template")
            })?;
            let cmd = tpl
                .replace("{cwd}", &cwd_s)
                .replace("{cmd}", shell_cmd)
                .replace("{title}", title);
            let child = spawn_shell_template(&cmd).context("spawn custom external terminal")?;
            Ok(Some(child.id()))
        }
    }
}

fn open_xdg_terminal(cwd_s: &str, shell_cmd: &str) -> Result<Option<u32>> {
    for bin in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        if which::which(bin).is_ok() {
            let child = if bin == "gnome-terminal" {
                Command::new(bin)
                    .args(["--working-directory", cwd_s, "--", "sh", "-c", shell_cmd])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
            } else if bin == "konsole" {
                Command::new(bin)
                    .args(["--workdir", cwd_s, "-e", "sh", "-c", shell_cmd])
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
                        &format!("cd {} && {}", shell_escape(cwd_s), shell_cmd),
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

fn spawn_shell_template(cmd: &str) -> std::io::Result<std::process::Child> {
    if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}

fn apple_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a default follow-logs command for a task (platform shell).
pub fn follow_logs_command(stdout_path: &Path, stderr_path: &Path) -> String {
    if cfg!(windows) {
        follow_logs_command_windows(stdout_path, stderr_path)
    } else {
        follow_logs_command_unix(stdout_path, stderr_path)
    }
}

fn follow_logs_command_unix(stdout_path: &Path, stderr_path: &Path) -> String {
    let out = stdout_path.display();
    let err = stderr_path.display();
    format!(
        "echo 'cco logs · {out}'; \
         (test -f '{out}' && tail -n +1 -f '{out}' || true) & \
         (test -f '{err}' && tail -n +1 -f '{err}' || true) & \
         wait"
    )
}

fn follow_logs_command_windows(stdout_path: &Path, stderr_path: &Path) -> String {
    let out = stdout_path.display().to_string().replace('\'', "''");
    let err = stderr_path.display().to_string().replace('\'', "''");
    format!(
        "Write-Host 'cco logs · {out}'; \
         Write-Host '(stderr: {err})'; \
         $p = '{out}'; \
         while (-not (Test-Path -LiteralPath $p)) {{ Start-Sleep -Seconds 1 }}; \
         Get-Content -LiteralPath $p -Wait -Tail 100"
    )
}

/// Interactive shell in work dir (platform shell keep-alive).
pub fn shell_in_dir_command() -> String {
    if cfg!(windows) {
        "Write-Host 'cco shell'".into()
    } else {
        "exec ${SHELL:-/bin/zsh} -l".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn prefer_windows_names() {
        assert_eq!(detect_launcher("wt"), ExternalLauncher::WindowsTerminal);
        assert_eq!(
            detect_launcher("windows_terminal"),
            ExternalLauncher::WindowsTerminal
        );
        assert_eq!(detect_launcher("powershell"), ExternalLauncher::PowerShell);
        assert_eq!(detect_launcher("pwsh"), ExternalLauncher::PowerShell);
        assert_eq!(detect_launcher("cmd"), ExternalLauncher::Cmd);
        assert_eq!(detect_launcher("kitty").as_str(), "kitty");
        assert_eq!(detect_launcher("iterm").as_str(), "iterm");
    }

    #[test]
    fn as_str_windows_variants() {
        assert_eq!(ExternalLauncher::WindowsTerminal.as_str(), "wt");
        assert_eq!(ExternalLauncher::PowerShell.as_str(), "powershell");
        assert_eq!(ExternalLauncher::Cmd.as_str(), "cmd");
    }

    #[test]
    fn follow_logs_command_nonempty() {
        let out = PathBuf::from("stdout.log");
        let err = PathBuf::from("stderr.log");
        let cmd = follow_logs_command(&out, &err);
        assert!(!cmd.is_empty());
        if cfg!(windows) {
            assert!(cmd.contains("Get-Content"), "win follow: {cmd}");
            assert!(cmd.contains("stdout.log"), "win follow path: {cmd}");
        } else {
            assert!(cmd.contains("tail"), "unix follow: {cmd}");
            assert!(cmd.contains("stdout.log"));
        }
    }

    #[test]
    fn shell_in_dir_nonempty() {
        assert!(!shell_in_dir_command().is_empty());
    }

    #[test]
    fn unknown_prefer_falls_to_platform_default() {
        let l = detect_launcher("not-a-real-launcher-xyz");
        if cfg!(windows) {
            assert_eq!(l, ExternalLauncher::Cmd);
        } else if cfg!(target_os = "macos") {
            assert_eq!(l, ExternalLauncher::AppleTerminal);
        } else {
            assert_eq!(l, ExternalLauncher::XdgTerminal);
        }
    }
}
