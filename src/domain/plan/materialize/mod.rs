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

mod role;
mod selected;

pub use role::{materialize_role_defaults};
pub use selected::materialize_selected_tasks;

use super::optional::normalize_optional_title;