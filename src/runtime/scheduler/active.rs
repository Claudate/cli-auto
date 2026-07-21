//! Active task set (`--only` / `--from-task`) — domain pure rules + PlanIR edges.
//!
//! [INPUT]: plan tasks · only/from filters
//! [OUTPUT]: HashSet of task ids that may run
//! [POS]: runtime/scheduler
//! [PROTOCOL]: 变更时更新 scheduler/mod.rs 头部

use std::collections::HashSet;

use anyhow::Result;

use crate::domain::run::{resolve_active_ids, ActiveFilter};

use super::Scheduler;

impl Scheduler {
    pub(super) fn active_task_ids(&self) -> Result<HashSet<String>> {
        let all: HashSet<String> = self.plan.tasks.iter().map(|t| t.id.clone()).collect();
        let edges: Vec<(String, Vec<String>)> = self
            .plan
            .tasks
            .iter()
            .map(|t| (t.id.clone(), t.depends_on.clone()))
            .collect();
        let filter = if let Some(only) = &self.only {
            ActiveFilter::Only(only.clone())
        } else if let Some(from) = &self.from_task {
            ActiveFilter::FromTask(from.clone())
        } else {
            ActiveFilter::All
        };
        resolve_active_ids(&all, &edges, &filter).map_err(|e| anyhow::anyhow!(e))
    }
}
