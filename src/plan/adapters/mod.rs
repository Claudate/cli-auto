//! Plan file adapters → PlanIR.
//!
//! [INPUT]: 无；子模块导出
//! [OUTPUT]: cco_v1 / raw_single / serial_prompts
//! [POS]: plan 适配器总线
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/adapters/CLAUDE.md

pub mod cco_v1;
pub mod raw_single;
pub mod serial_prompts;
