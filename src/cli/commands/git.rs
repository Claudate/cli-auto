//! CLI `cco git` subcommand handlers.
//!
//! [INPUT]: Config · clap GitCommands fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；薄壳，调 services::git
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::cli::{GitCommands, GitIdentityCommands, GitRemoteCommands};
use crate::config::{normalize_region, region_label, Config};
use crate::services::git as git_svc;

pub fn run(config: &Config, cmd: GitCommands) -> Result<i32> {
    match cmd {
        GitCommands::Status { project } => {
            let proj = resolve_project(project)?;
            let v = git_svc::status(config, &proj)?;
            println!("repo:     {}", if v.is_repo { "yes" } else { "no" });
            if let Some(b) = &v.branch {
                println!("branch:   {b}");
            }
            if let Some(u) = &v.upstream {
                println!("upstream: {u}");
            }
            println!("clean:    {}", if v.clean { "yes" } else { "no" });
            if !v.changes.is_empty() {
                println!("changes ({}):", v.changes.len());
                for c in &v.changes {
                    println!("  {c}");
                }
            }
            if !v.configured_remotes.is_empty() {
                println!("\nconfigured remotes:");
                for r in &v.configured_remotes {
                    println!(
                        "  {:<10} {} [{}] {}",
                        r.name,
                        r.url,
                        r.region_label,
                        r.note.as_deref().unwrap_or("")
                    );
                }
            }
            if !v.actual_remotes.is_empty() {
                println!("\nactual remotes:");
                for r in &v.actual_remotes {
                    let mark = if r.configured { "✓" } else { " " };
                    println!("  [{mark}] {:<10} {}", r.name, r.url);
                }
            }
            if let Some(n) = &v.user_name {
                println!("\nidentity: {n} <{}>", v.user_email.as_deref().unwrap_or(""));
            } else {
                println!("\nidentity: (not set)");
            }
            Ok(0)
        }

        GitCommands::Remote { cmd } => run_remote(config, cmd),

        GitCommands::Identity { cmd } => run_identity(config, cmd),

        GitCommands::Commit {
            project,
            message,
            dry_run,
            push,
            all,
            paths,
            force,
        } => {
            let proj = resolve_project(project)?;
            if message.is_none() && !dry_run {
                bail!("--message is required (or use --dry-run)");
            }
            let msg = message.unwrap_or_default();
            let r = git_svc::commit(config, &proj, &msg, dry_run, push, all, &paths, force)?;
            println!("{}", r.message);
            if !r.files.is_empty() {
                println!("files:");
                for f in &r.files {
                    println!("  {f}");
                }
            }
            if let Some(h) = &r.commit_hash {
                println!("commit: {h}");
            }
            if r.pushed {
                if let Some(o) = &r.push_output {
                    println!("push: {o}");
                }
            } else if push && r.commit_hash.is_some() {
                if let Some(o) = &r.push_output {
                    println!("push: {o}");
                }
            }
            Ok(0)
        }

        GitCommands::Push {
            project,
            remote,
            branch,
            force,
        } => {
            let proj = resolve_project(project)?;
            let r = git_svc::push(config, &proj, remote.as_deref(), branch.as_deref(), force)?;
            println!("{}", r.message);
            if let Some(o) = &r.output {
                println!("{o}");
            }
            Ok(0)
        }

        GitCommands::Doctor { project } => {
            let proj = resolve_project(project)?;
            let lines = git_svc::doctor(config, &proj)?;
            let mut all_ok = true;
            for l in &lines {
                let mark = if l.ok { "ok" } else { "FAIL" };
                println!("  [{mark}] {:<22} {}", l.name, l.detail);
                if !l.ok {
                    all_ok = false;
                }
            }
            if all_ok {
                println!("\ngit doctor: all checks passed");
                Ok(0)
            } else {
                println!("\ngit doctor: some checks failed");
                Ok(1)
            }
        }
    }
}

fn run_remote(config: &Config, cmd: GitRemoteCommands) -> Result<i32> {
    match cmd {
        GitRemoteCommands::List => {
            if config.git.remotes.is_empty() {
                println!("(no configured remotes; use `cco git remote add`)");
            } else {
                for r in &config.git.remotes {
                    let region = region_label(&r.region);
                    println!(
                        "{:<10} {} [{}] {}",
                        r.name,
                        r.url,
                        region,
                        r.note.as_deref().unwrap_or("")
                    );
                }
            }
            Ok(0)
        }
        GitRemoteCommands::Add {
            name,
            url,
            region,
            note,
        } => {
            let reg = normalize_region(&region)
                .ok_or_else(|| anyhow::anyhow!("invalid region: {region} (use domestic|overseas)"))?;
            let mut cfg = config.clone();
            git_svc::add_remote(&mut cfg, &name, &url, reg, note)?;
            println!("added remote {name} → {url} [{region}]");
            Ok(0)
        }
        GitRemoteCommands::Remove { name } => {
            let mut cfg = config.clone();
            let removed = git_svc::remove_remote(&mut cfg, &name)?;
            if removed {
                println!("removed remote {name}");
                Ok(0)
            } else {
                bail!("remote {name} not found in config");
            }
        }
        GitRemoteCommands::Apply { project } => {
            let proj = resolve_project(project)?;
            let actions = git_svc::apply_remotes(config, &proj)?;
            for a in &actions {
                println!("{a}");
            }
            Ok(0)
        }
    }
}

fn run_identity(config: &Config, cmd: GitIdentityCommands) -> Result<i32> {
    match cmd {
        GitIdentityCommands::Set {
            project,
            name,
            email,
        } => {
            let proj = resolve_project(project)?;
            git_svc::set_identity(&proj, name.as_deref(), email.as_deref())?;
            match (&name, &email) {
                (Some(n), Some(e)) => println!("set identity: {n} <{e}>"),
                (Some(n), None) => println!("set name: {n}"),
                (None, Some(e)) => println!("set email: {e}"),
                (None, None) => println!("(no changes)"),
            }
            Ok(0)
        }
        GitIdentityCommands::Show { project } => {
            let proj = resolve_project(project)?;
            let v = git_svc::status(config, &proj)?;
            match (&v.user_name, &v.user_email) {
                (Some(n), Some(e)) => println!("{n} <{e}>"),
                (Some(n), None) => println!("{n} (no email)"),
                (None, Some(e)) => println!("(no name) <{e}>"),
                (None, None) => println!("(not set)"),
            }
            Ok(0)
        }
    }
}

/// Resolve project path: explicit arg → cwd.
fn resolve_project(project: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = project {
        if p.is_dir() {
            return Ok(p);
        }
        bail!("project directory does not exist: {}", p.display());
    }
    // Default: current directory.
    let cwd = std::env::current_dir().context("get current dir")?;
    Ok(cwd)
}
