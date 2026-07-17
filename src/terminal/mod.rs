//! Multi-terminal sessions: external system windows (+ embedded placeholder for TUI).

pub mod external;
pub mod manager;

pub use external::{detect_launcher, ExternalLauncher};
pub use manager::{SessionKind, TerminalManager, TerminalSession};
