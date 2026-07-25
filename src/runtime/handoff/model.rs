//! Handoff ledger model + md/json render (A1-5 adapter).
//!
//! [INPUT]: PlanIR · RunState
//! [OUTPUT]: Handoff shell · BoardRow · Fragment · load/save
//! [POS]: runtime/handoff — disk schema `cco-handoff/v1` (do not silently change)
//! [PROTOCOL]: schema 变更须同步 docs/contracts/run-dir.md

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::plan::{PlanIR, TaskIR, TaskRole};
use crate::runtime::provider::TaskStatus;
use crate::state::RunState;

pub const HANDOFF_SCHEMA: &str = "cco-handoff/v1";

/// Marker wrapping the host-injected handoff summary on task start (P1-5).
pub const HANDOFF_PROMPT_OPEN: &str = "[CCO_HANDOFF]";
pub const HANDOFF_PROMPT_CLOSE: &str = "[/CCO_HANDOFF]";

/// Max chars for each depends_on fragment summary inside the prompt prefix.
pub(super) const PREFIX_SUMMARY_CHARS: usize = 200;

/// Conventional per-task changed-files product (P2-2, relative to work_dir).
pub const TASK_CHANGED_REL: &str = ".cco-out/{task_id}/CHANGED.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub schema: String,
    pub run_id: String,
    pub updated: DateTime<Utc>,
    pub project: String,
    pub plan: String,
    pub status: String,
    pub board: Vec<BoardRow>,
    #[serde(default)]
    pub timeline: Vec<String>,
    #[serde(default)]
    pub fragments: BTreeMap<String, Fragment>,
    #[serde(default)]
    pub open_risks: Vec<String>,
    #[serde(default)]
    pub instructions_for_next: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardRow {
    pub id: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub status: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Fragment {
    pub status: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

impl Handoff {
    pub fn path_md(run_dir: &Path) -> PathBuf {
        run_dir.join("handoff.md")
    }

    pub fn path_json(run_dir: &Path) -> PathBuf {
        run_dir.join("handoff.json")
    }

    /// Empty shell at run start: Board = all pending.
    pub fn init_shell(plan: &PlanIR, state: &RunState) -> Self {
        let board = plan
            .tasks
            .iter()
            .map(|t| board_row_from_task(t, "pending", None, String::new()))
            .collect();
        Self {
            schema: HANDOFF_SCHEMA.into(),
            run_id: state.run_id.clone(),
            updated: Utc::now(),
            project: state.project_root.display().to_string(),
            plan: state.plan_path.display().to_string(),
            status: "running".into(),
            board,
            timeline: vec![format!(
                "{} · run_start · shell created",
                Utc::now().to_rfc3339()
            )],
            fragments: BTreeMap::new(),
            open_risks: vec![],
            instructions_for_next: default_next_instructions(plan, &[]),
        }
    }

    pub fn load(run_dir: &Path) -> Result<Self> {
        let path = Self::path_json(run_dir);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, run_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(run_dir)?;
        let json_path = Self::path_json(run_dir);
        std::fs::write(&json_path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("write {}", json_path.display()))?;
        let md_path = Self::path_md(run_dir);
        std::fs::write(&md_path, self.render_md())
            .with_context(|| format!("write {}", md_path.display()))?;
        Ok(())
    }

    pub fn render_md(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# CCO Handoff · run_id={}\n", self.run_id));
        md.push_str(&format!("updated: {}\n", self.updated.to_rfc3339()));
        md.push_str(&format!("project: {}\n", self.project));
        md.push_str(&format!("plan: {}\n", self.plan));
        md.push_str(&format!("status: {}\n\n", self.status));

        md.push_str("## Board\n");
        md.push_str("| id | provider | role | status | scope | outputs | cost | notes |\n");
        md.push_str("|----|----------|------|--------|-------|---------|------|-------|\n");
        for r in &self.board {
            let role = r.role.as_deref().unwrap_or("-");
            let cost = r
                .cost
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into());
            let outs = if r.outputs.is_empty() {
                "-".into()
            } else {
                r.outputs.join(", ")
            };
            let scope = if r.scope.is_empty() {
                "-"
            } else {
                r.scope.as_str()
            };
            let notes = if r.notes.is_empty() {
                "-"
            } else {
                r.notes.as_str()
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                r.id, r.provider, role, r.status, scope, outs, cost, notes
            ));
        }
        md.push('\n');

        md.push_str("## Timeline\n");
        if self.timeline.is_empty() {
            md.push_str("- (empty)\n");
        } else {
            for line in &self.timeline {
                md.push_str(&format!("- {line}\n"));
            }
        }
        md.push('\n');

        md.push_str("## Fragments\n");
        if self.fragments.is_empty() {
            md.push_str("_none yet_\n");
        } else {
            for (id, f) in &self.fragments {
                md.push_str(&format!("### {id}\n"));
                md.push_str(&format!(
                    "- status: {} · provider: {}\n",
                    f.status, f.provider
                ));
                if let Some(wd) = &f.work_dir {
                    md.push_str(&format!("- work_dir: `{wd}`\n"));
                }
                if let Some(br) = &f.branch {
                    md.push_str(&format!("- branch: `{br}`\n"));
                }
                if !f.summary.is_empty() {
                    md.push_str(&format!("- summary: {}\n", f.summary));
                }
                if !f.artifacts.is_empty() {
                    md.push_str(&format!("- artifacts: {}\n", f.artifacts.join(", ")));
                }
                if !f.risks.is_empty() {
                    md.push_str(&format!("- risks: {}\n", f.risks.join("; ")));
                }
                md.push('\n');
            }
        }

        md.push_str("## Open risks\n");
        if self.open_risks.is_empty() {
            md.push_str("- (none)\n");
        } else {
            for r in &self.open_risks {
                md.push_str(&format!("- {r}\n"));
            }
        }
        md.push('\n');

        md.push_str("## Instructions for next worker\n");
        if self.instructions_for_next.is_empty() {
            md.push_str("- read Board + Fragments; respect scope\n");
        } else {
            md.push_str(&self.instructions_for_next);
            if !self.instructions_for_next.ends_with('\n') {
                md.push('\n');
            }
        }
        md
    }

    pub(super) fn set_board_status(
        &mut self,
        task_id: &str,
        status: &str,
        cost: Option<f64>,
        notes: &str,
    ) {
        if let Some(row) = self.board.iter_mut().find(|r| r.id == task_id) {
            row.status = status.into();
            if cost.is_some() {
                row.cost = cost;
            }
            if !notes.is_empty() {
                row.notes = notes.into();
            }
        }
    }

    pub(super) fn push_timeline(&mut self, line: impl Into<String>) {
        self.timeline.push(line.into());
        // keep timeline bounded
        if self.timeline.len() > 200 {
            let drop_n = self.timeline.len() - 200;
            self.timeline.drain(0..drop_n);
        }
    }
}

pub(super) fn role_str(role: Option<TaskRole>) -> Option<String> {
    role.map(|r| r.as_str().to_string())
}

pub(super) fn scope_summary(task: &TaskIR) -> String {
    let Some(s) = &task.scope else {
        return String::new();
    };
    let mut parts = Vec::new();
    if !s.paths.is_empty() {
        parts.push(format!("w:{}", s.paths.join(",")));
    }
    if !s.readonly.is_empty() {
        parts.push(format!("r:{}", s.readonly.join(",")));
    }
    if !s.forbid.is_empty() {
        parts.push(format!("!:{}", s.forbid.join(",")));
    }
    parts.join(" ")
}

fn board_row_from_task(
    task: &TaskIR,
    status: &str,
    cost: Option<f64>,
    notes: String,
) -> BoardRow {
    BoardRow {
        id: task.id.clone(),
        provider: task.provider.clone(),
        role: role_str(task.role),
        status: status.into(),
        scope: scope_summary(task),
        outputs: task.outputs.clone(),
        cost,
        notes,
    }
}

pub(super) fn default_next_instructions(plan: &PlanIR, done: &[String]) -> String {
    let pending: Vec<&str> = plan
        .tasks
        .iter()
        .filter(|t| !done.iter().any(|d| d == &t.id))
        .map(|t| t.id.as_str())
        .collect();
    if pending.is_empty() {
        "- all tasks terminal; no next worker\n".into()
    } else {
        format!(
            "- ready candidates (check depends_on): {}\n- read Board + Fragments of depends_on\n- write only your outputs; do not edit global handoff\n",
            pending.join(", ")
        )
    }
}

pub(super) fn status_label(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Queued => "queued",
        TaskStatus::Starting => "starting",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Timeout => "timeout",
        TaskStatus::Stopped => "stopped",
        TaskStatus::Skipped => "skipped",
    }
}
