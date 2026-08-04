//! CLI `cco git` subcommand handlers.
//!
//! [INPUT]: Config · clap GitCommands / GitBranchCommands / GitStashCommands fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；薄壳，调 services::git
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::cli::{
    GitBranchCommands, GitCommands, GitIdentityCommands, GitRemoteCommands, GitStashCommands,
};
use crate::config::{normalize_region, region_label, Config};
use crate::services::git as git_svc;

/// Pull strategy from CLI string.
fn parse_strategy(s: &str) -> git_svc::PullStrategy {
    match s.trim().to_ascii_lowercase().as_str() {
        "merge" => git_svc::PullStrategy::Merge,
        "fail" => git_svc::PullStrategy::Fail,
        _ => git_svc::PullStrategy::Rebase,
    }
}

pub fn run(config: &Config, cmd: GitCommands) -> Result<i32> {
    match cmd {
        GitCommands::Status { project } => {
            let proj = resolve_project(project)?;
            let v = git_svc::status(config, &proj)?;
            println!("repo:     {}", if v.is_repo { "yes" } else { "no" });
            if let Some(b) = &v.branch { println!("branch:   {b}"); }
            if let Some(u) = &v.upstream { println!("upstream: {u}"); }
            println!("clean:    {}", if v.clean { "yes" } else { "no" });
            if !v.changes.is_empty() {
                println!("changes ({}):", v.changes.len());
                for c in &v.changes { println!("  {c}"); }
            }
            if !v.configured_remotes.is_empty() {
                println!("\nconfigured remotes:");
                for r in &v.configured_remotes {
                    println!("  {:<10} {} [{}] {}", r.name, r.url, r.region_label, r.note.as_deref().unwrap_or(""));
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

        GitCommands::Commit { project, message, dry_run, push, all, paths, force } => {
            let proj = resolve_project(project)?;
            if message.is_none() && !dry_run { bail!("--message is required (or use --dry-run)"); }
            let msg = message.unwrap_or_default();
            let r = git_svc::commit(config, &proj, &msg, dry_run, push, all, &paths, force)?;
            println!("{}", r.message);
            if !r.files.is_empty() {
                println!("files:");
                for f in &r.files { println!("  {f}"); }
            }
            if let Some(h) = &r.commit_hash { println!("commit: {h}"); }
            if r.pushed {
                if let Some(o) = &r.push_output { println!("push: {o}"); }
            } else if push && r.commit_hash.is_some() {
                if let Some(o) = &r.push_output { println!("push: {o}"); }
            }
            Ok(0)
        }

        GitCommands::Push { project, remote, branch, force } => {
            let proj = resolve_project(project)?;
            let r = git_svc::push(config, &proj, remote.as_deref(), branch.as_deref(), force)?;
            println!("{}", r.message);
            if let Some(o) = &r.output { println!("{o}"); }
            Ok(0)
        }

        GitCommands::Doctor { project } => {
            let proj = resolve_project(project)?;
            let lines = git_svc::doctor(config, &proj)?;
            let mut all_ok = true;
            for l in &lines {
                let mark = if l.ok { "ok" } else { "FAIL" };
                println!("  [{mark}] {:<22} {}", l.name, l.detail);
                if !l.ok { all_ok = false; }
            }
            if all_ok { println!("\ngit doctor: all checks passed"); Ok(0) }
            else { println!("\ngit doctor: some checks failed"); Ok(1) }
        }

        GitCommands::Pull { project, remote, branch, strategy } => {
            let proj = resolve_project(project)?;
            let strat = parse_strategy(&strategy);
            let r = git_svc::pull(config, &proj, remote.as_deref(), branch.as_deref(), strat)?;
            println!("{}", r.message);
            if r.merged { println!("files changed: {}", r.files_changed); }
            if let Some(o) = &r.output { println!("{o}"); }
            Ok(0)
        }

        GitCommands::Fetch { project, remote, prune } => {
            let proj = resolve_project(project)?;
            let r = git_svc::fetch(&proj, remote.as_deref(), prune)?;
            println!("{}", r.message);
            if let Some(o) = &r.output { println!("{o}"); }
            Ok(0)
        }

        GitCommands::Log { project, n, oneline } => {
            let proj = resolve_project(project)?;
            if oneline {
                let out = git_svc::log_oneline(&proj, Some(n))?;
                println!("{out}");
            } else {
                let entries = git_svc::log(&proj, Some(n))?;
                for e in &entries {
                    println!("{}  {}  {}", &e.hash[..8.min(e.hash.len())], e.date, e.message);
                    if !e.author.is_empty() { println!("         author: {}", e.author); }
                }
            }
            Ok(0)
        }

        GitCommands::Diff { project, staged, stat, name_only } => {
            let proj = resolve_project(project)?;
            if name_only {
                let files = git_svc::diff_name_only(&proj)?;
                for f in &files { println!("{f}"); }
            } else if stat {
                let out = git_svc::diff_stat(&proj)?;
                println!("{out}");
            } else if staged {
                let out = git_svc::diff_staged(&proj)?;
                println!("{out}");
            } else {
                let out = git_svc::diff(&proj)?;
                println!("{out}");
            }
            Ok(0)
        }

        GitCommands::Stash { cmd } => run_stash(config, cmd),
        GitCommands::Branch { cmd } => run_branch(config, cmd),
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
                    println!("{:<10} {} [{}] {}", r.name, r.url, region, r.note.as_deref().unwrap_or(""));
                }
            }
            Ok(0)
        }
        GitRemoteCommands::Add { name, url, region, note } => {
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
            if removed { println!("removed remote {name}"); Ok(0) }
            else { bail!("remote {name} not found in config"); }
        }
        GitRemoteCommands::Apply { project } => {
            let proj = resolve_project(project)?;
            let actions = git_svc::apply_remotes(config, &proj)?;
            for a in &actions { println!("{a}"); }
            Ok(0)
        }
    }
}

fn run_identity(config: &Config, cmd: GitIdentityCommands) -> Result<i32> {
    match cmd {
        GitIdentityCommands::Set { project, name, email } => {
            let proj = resolve_project(project)?;
            git_svc::set_identity(&proj, name.as_deref(), email.as_deref())?;
            println!("identity set");
            Ok(0)
        }
        GitIdentityCommands::Show { project } => {
            let proj = resolve_project(project)?;
            let v = git_svc::status(config, &proj)?;
            if let Some(n) = &v.user_name {
                println!("{} <{}>", n, v.user_email.as_deref().unwrap_or(""));
            } else {
                println!("(no identity set)");
            }
            Ok(0)
        }
    }
}

fn run_branch(_config: &Config, cmd: GitBranchCommands) -> Result<i32> {
    match cmd {
        GitBranchCommands::List { project } => {
            let proj = resolve_project(project)?;
            let branches = git_svc::list_branches(&proj)?;
            for b in &branches {
                let mark = if b.current { "*" } else { " " };
                let up = b.upstream.as_deref().map(|u| format!(" → {u}")).unwrap_or_default();
                println!("{mark} {}{up}", b.name);
            }
            Ok(0)
        }
        GitBranchCommands::Create { project, name, base } => {
            let proj = resolve_project(project)?;
            let msg = git_svc::create_branch(&proj, &name, base.as_deref())?;
            println!("{msg}");
            Ok(0)
        }
        GitBranchCommands::Switch { project, name } => {
            let proj = resolve_project(project)?;
            let msg = git_svc::switch_branch(&proj, &name)?;
            println!("{msg}");
            Ok(0)
        }
        GitBranchCommands::Delete { project, name, force } => {
            let proj = resolve_project(project)?;
            let msg = git_svc::delete_branch(&proj, &name, force)?;
            println!("{msg}");
            Ok(0)
        }
    }
}

fn run_stash(_config: &Config, cmd: GitStashCommands) -> Result<i32> {
    match cmd {
        GitStashCommands::List => {
            let proj = std::env::current_dir().context("current dir")?;
            let entries = git_svc::stash_list(&proj)?;
            for e in &entries { println!("stash@{{{}}}: {}", e.index, e.message); }
            Ok(0)
        }
    }
}

fn resolve_project(project: Option<PathBuf>) -> Result<PathBuf> {
    match project {
        Some(p) => Ok(p),
        None => std::env::current_dir().context("no --project and no current dir"),
    }
}
