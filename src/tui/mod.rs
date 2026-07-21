//! Multi-page TUI (ratatui). Observes a run via app::run queries; light stop control.
//!
//! [INPUT]: 无；re-export
//! [OUTPUT]: run_tui · TuiOptions · options_from_config
//! [POS]: TUI 模块入口 · Presentation（A5-3 只读 app + 轻控制）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/tui/CLAUDE.md

pub mod app;
mod pages;
mod widgets;

pub use app::{options_from_config, run_tui, TuiOptions};
