//! Shared shell-print WorkerPort skeleton (codex + multi-CLI profiles).
//!
//! [INPUT]: ShellProfile · config bin/extra_args
//! [OUTPUT]: ShellPrintProvider · scope prefix helpers · stream_child
//! [POS]: runtime/provider/shell_print — adapters only; no soft-fill / failover policy
//! [PROTOCOL]: 变更时更新此头部与 runtime/provider/CLAUDE.md；禁止 spawn 时网络安装

pub mod adapter;
pub mod decode;
pub mod profiles;
pub mod scope;
pub mod stream;

pub use adapter::ShellPrintProvider;
pub use profiles::{
    profile_by_name, provider_docs_url, ResultKind, ShellProfile, ALL_SHELL_PROFILES, CODEBUDDY,
    CODEX, COPILOT, DEEPSEEK, GEMINI, KIMI, QWEN,
};
pub use scope::{build_scope_prefix, with_scope_prefix};
pub use stream::{process_alive, stop_pid, stream_child};
