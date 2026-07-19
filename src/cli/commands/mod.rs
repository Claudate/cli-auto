//! CLI subcommand handlers (D4 split from cli/mod.rs).
//!
//! [INPUT]: Config · clap Commands fields
//! [OUTPUT]: per-command exit code
//! [POS]: cli 命令实现；mod.rs 仅保留枚举与 dispatch
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

pub mod common;
pub mod doctor;
pub mod init;
pub mod logs;
pub mod parse;
pub mod plan_cmd;
pub mod plans;
pub mod report;
pub mod resume;
pub mod run;
pub mod status;
pub mod stop;
pub mod term;
pub mod tui_cmd;
