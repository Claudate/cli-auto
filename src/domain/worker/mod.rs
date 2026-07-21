//! Worker domain: pure route / capability / isolation / failover policy (A1-4).
//!
//! **No** process spawn, **no** path layout for worktrees, **no** preflight IO.
//!
//! [INPUT]: provider name strings · plan fields · failover flags
//! [OUTPUT]: fill decisions · isolation mode · failover target name
//! [POS]: domain/worker — scheduler + CLI call these; adapters do IO
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；soft-fill **不得**静默盖显式 route

mod failover;
mod isolation;
mod route;
mod types;

pub use failover::FailoverPolicy;
pub use isolation::{is_multi_provider, isolation_on_fail, IsolationOnFail};
pub use route::{
    apply_route_fill, apply_worker_defaults, is_still_default_route, RouteFillMode, RouteFillReport,
};
pub use types::{CapabilityFlags, ProviderId, WorkerRoute};
