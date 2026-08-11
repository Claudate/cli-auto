use super::{normalize_optional_title, materialize_role_defaults};
use anyhow::{bail, Result};
use std::collections::HashSet;

use crate::domain::plan::types::PlanIR;

// Drop unselected optional tasks and rewrite depends_on for execution.
pub fn materialize_selected_tasks(mut ir: PlanIR) -> Result<PlanIR> {
    for t in &mut ir.tasks {
        if !t.optional {
            t.include = true;
        }
        if t.optional {
            t.title = normalize_optional_title(&t.title, true);
        }
    }
    let drop: HashSet<String> = ir
        .tasks
        .iter()
        .filter(|t| !t.include)
        .map(|t| t.id.clone())
        .collect();
    ir.tasks.retain(|t| t.include);
    if ir.tasks.is_empty() {
        bail!("没有选中任何任务：请至少保留一项必选，或勾选可选项后再开始");
    }
    for t in &mut ir.tasks {
        t.depends_on.retain(|d| !drop.contains(d));
    }
    // Role defaults already applied at load_plan; re-apply is idempotent if IR
    // was built in-memory (planner / tests) without going through load_plan.
    materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}
