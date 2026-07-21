//! Windows native external terminal spawn helpers (P2-7 thin slice).
//!
//! [INPUT]: cwd · shell_cmd (PowerShell / cmd dialect)
//! [OUTPUT]: open wt/powershell/cmd · cmdline preview
//! [POS]: called from external::open_window on Windows launchers
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/terminal/CLAUDE.md

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use super::external::ExternalLauncher;

/// Windows Terminal: `wt.exe -d <cwd> -- powershell -NoExit -Command <cmd>`.
pub(super) fn open_windows_terminal(cwd_s: &str, shell_cmd: &str) -> Result<Option<u32>> {
    let wt = which::which("wt.exe")
        .or_else(|_| which::which("wt"))
        .context("Windows Terminal (wt.exe) not on PATH")?;
    let status = Command::new(wt)
        .args([
            "-d",
            cwd_s,
            "--",
            "powershell",
            "-NoExit",
            "-NoProfile",
            "-Command",
            shell_cmd,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn wt.exe")?;
    Ok(Some(status.id()))
}

/// Standalone PowerShell window (no Windows Terminal required).
pub(super) fn open_powershell_window(cwd_s: &str, shell_cmd: &str) -> Result<Option<u32>> {
    let ps = which::which("pwsh.exe")
        .or_else(|_| which::which("pwsh"))
        .or_else(|_| which::which("powershell.exe"))
        .or_else(|_| which::which("powershell"))
        .context("powershell not on PATH")?;
    let inner = format!(
        "Set-Location -LiteralPath {}; {}",
        ps_single_quote(cwd_s),
        shell_cmd
    );
    let status = Command::new(ps)
        .args(["-NoExit", "-NoProfile", "-Command", &inner])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn powershell")?;
    Ok(Some(status.id()))
}

/// `cmd /C start "title" cmd /K "cd /d cwd && shell_cmd"`.
pub(super) fn open_cmd_window(cwd_s: &str, shell_cmd: &str) -> Result<Option<u32>> {
    let line = format!("cd /d {} && {}", cmd_escape(cwd_s), shell_cmd);
    let status = Command::new("cmd")
        .args(["/C", "start", "cco", "cmd", "/K", &line])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn cmd start")?;
    Ok(Some(status.id()))
}

/// Pure cmdline preview for tests / dry-run (does not spawn).
pub fn windows_cmdline_preview(
    launcher: ExternalLauncher,
    cwd: &Path,
    shell_cmd: &str,
) -> Option<(String, Vec<String>)> {
    let cwd_s = cwd.display().to_string();
    match launcher {
        ExternalLauncher::WindowsTerminal => Some((
            "wt".into(),
            vec![
                "-d".into(),
                cwd_s,
                "--".into(),
                "powershell".into(),
                "-NoExit".into(),
                "-NoProfile".into(),
                "-Command".into(),
                shell_cmd.into(),
            ],
        )),
        ExternalLauncher::PowerShell => {
            let inner = format!(
                "Set-Location -LiteralPath {}; {}",
                ps_single_quote(&cwd_s),
                shell_cmd
            );
            Some((
                "powershell".into(),
                vec![
                    "-NoExit".into(),
                    "-NoProfile".into(),
                    "-Command".into(),
                    inner,
                ],
            ))
        }
        ExternalLauncher::Cmd => {
            let line = format!("cd /d {} && {}", cmd_escape(&cwd_s), shell_cmd);
            Some((
                "cmd".into(),
                vec![
                    "/C".into(),
                    "start".into(),
                    "cco".into(),
                    "cmd".into(),
                    "/K".into(),
                    line,
                ],
            ))
        }
        _ => None,
    }
}

/// PowerShell single-quoted string: double embedded `'`.
fn ps_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Minimal cmd metachar escape for paths in `cd /d …`.
fn cmd_escape(s: &str) -> String {
    if s.chars()
        .any(|c| c.is_whitespace() || "^&|<>()!".contains(c))
    {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn windows_cmdline_preview_wt() {
        let cwd = PathBuf::from(r"C:\proj");
        let (prog, args) =
            windows_cmdline_preview(ExternalLauncher::WindowsTerminal, &cwd, "Get-Content log")
                .expect("wt preview");
        assert_eq!(prog, "wt");
        assert!(args.iter().any(|a| a == "-d"));
        assert!(args.iter().any(|a| a == "powershell"));
        assert!(args.iter().any(|a| a == "-NoExit"));
        assert!(args.iter().any(|a| a == "Get-Content log"));
    }

    #[test]
    fn windows_cmdline_preview_cmd() {
        let cwd = PathBuf::from(r"C:\work dir");
        let (prog, args) =
            windows_cmdline_preview(ExternalLauncher::Cmd, &cwd, "dir").expect("cmd preview");
        assert_eq!(prog, "cmd");
        let joined = args.join(" ");
        assert!(joined.contains("start"));
        assert!(joined.contains("/K"));
        assert!(joined.contains("cd /d"));
        assert!(joined.contains("dir"));
    }
}
