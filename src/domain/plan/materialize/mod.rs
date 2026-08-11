//! Materialize selected tasks + role defaults (pure, in-memory).
//!
//! Split by pure-function boundary (arch hard ≤600):
//! - [`selected`] — `materialize_selected_tasks`
//! - [`role`] — `materialize_role_defaults` + helpers
//!
//! [INPUT]: PlanIR
//! [OUTPUT]: materialize_selected_tasks · materialize_role_defaults
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部；inspect 空 depends_on 接线变更须同步单测
//!
//! Hard debt: 663→360 prod + tests.rs (hard 600 cleared)

mod role;
mod selected;

pub use role::materialize_role_defaults;
pub use selected::materialize_selected_tasks;

// Crate-private helpers used by soften + tests.
#[cfg(test)]
pub(crate) use role::materialize_inspect_task;
pub(crate) use role::wire_empty_inspect_depends_on;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
