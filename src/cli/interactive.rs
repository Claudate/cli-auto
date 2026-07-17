//! Interactive project / plan selection helpers.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::plan;

pub fn prompt_line(label: &str) -> Result<String> {
    eprint!("{label}");
    let _ = io::stderr().flush();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

pub fn confirm(question: &str, default_yes: bool) -> Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    eprint!("{question} {hint} ");
    let _ = io::stderr().flush();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let t = buf.trim();
    if t.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(t, "y" | "Y" | "yes" | "YES"))
}

/// Resolve project path: explicit, or interactive prompt.
pub fn resolve_project(explicit: Option<PathBuf>, require_explicit_noninteractive: bool) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if require_explicit_noninteractive && !atty_stderr() {
        bail!("--project is required in non-interactive mode");
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    eprintln!("project_root not set.");
    eprintln!("  current directory: {}", cwd.display());
    let line = prompt_line(&format!("project path [{}]: ", cwd.display()))?;
    let path = if line.is_empty() {
        cwd
    } else {
        PathBuf::from(line)
    };
    if !confirm(&format!("use project {} ?", path.display()), true)? {
        bail!("aborted (project not confirmed)");
    }
    Ok(path)
}

/// Resolve plan path: explicit, or list & pick under project.
pub fn resolve_plan(
    project: &Path,
    explicit: Option<PathBuf>,
    require_explicit_noninteractive: bool,
) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if require_explicit_noninteractive && !atty_stderr() {
        bail!("--plan is required in non-interactive mode");
    }
    let plans = plan::list_plans(project)?;
    if plans.is_empty() {
        let line = prompt_line("no plans found; enter plan path (relative or absolute): ")?;
        if line.is_empty() {
            bail!("no plan path provided");
        }
        return Ok(PathBuf::from(line));
    }
    eprintln!("plans under {}:", project.display());
    for (i, p) in plans.iter().enumerate() {
        let rel = p
            .strip_prefix(project)
            .map(|r| r.display().to_string())
            .unwrap_or_else(|_| p.display().to_string());
        eprintln!("  [{}] {}", i + 1, rel);
    }
    let line = prompt_line(&format!("select plan [1-{}] or path: ", plans.len()))?;
    if line.is_empty() {
        return Ok(plans[0].clone());
    }
    if let Ok(n) = line.parse::<usize>() {
        if n >= 1 && n <= plans.len() {
            return Ok(plans[n - 1].clone());
        }
        bail!("invalid plan index: {n}");
    }
    Ok(PathBuf::from(line))
}

fn atty_stderr() -> bool {
    // avoid extra dep: treat missing TTY as non-interactive via isatty
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn isatty(fd: i32) -> i32;
            }
            isatty(2) == 1
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn ensure_project_dir(project: &Path) -> Result<()> {
    if !project.exists() {
        bail!("project_root does not exist: {}", project.display());
    }
    if !project.is_dir() {
        bail!("project_root is not a directory: {}", project.display());
    }
    let _ = std::fs::read_dir(project)
        .with_context(|| format!("cannot read project_root {}", project.display()))?;
    Ok(())
}
