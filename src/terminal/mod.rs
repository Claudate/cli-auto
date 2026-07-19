//! Multi-terminal sessions: external system windows (+ embedded placeholder for TUI).
//!
//! [INPUT]: 无；re-export
//! [OUTPUT]: TerminalManager · SessionKind · ExternalLauncher
//! [POS]: terminal 模块入口；桌面未接（P1-2）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/terminal/CLAUDE.md

pub mod external;
pub mod manager;

pub use external::{detect_launcher, ExternalLauncher};
pub use manager::{SessionKind, TerminalManager, TerminalSession};
