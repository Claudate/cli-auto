//! Multi-page TUI (ratatui). Observes a run directory; optional live attach.

pub mod app;
mod pages;
mod widgets;

pub use app::{options_from_config, run_tui, TuiOptions};
