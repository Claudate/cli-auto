//! Shared human one-liner for run / plan-job status (H1).
//!
//! [INPUT]: snake_case status · task snapshots · optional plan-job fields
//! [OUTPUT]: StatusOneLiner（唯一主句 text · phase · done/total）
//! [POS]: domain/run · 纯投影；禁止 LLM；禁止写 STATE.md
//! [PROTOCOL]: 变更时更新此头部与 domain/run/mod.rs · H1 计划

use serde::Serialize;

/// High-level phase for UI badges (shell-level; not a fourth split-desk concept).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusPhase {
    Planning,
    AwaitConfirm,
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
    Idle,
}

/// Cross CLI / desktop / TUI status sentence (one source of truth for `text`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusOneLiner {
    pub phase: StatusPhase,
    /// Unique main-path human sentence (no bare run_id / VERDICT / engine name).
    pub text: String,
    pub done: u32,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_hint: Option<String>,
}

impl StatusOneLiner {
    pub fn idle() -> Self {
        Self {
            phase: StatusPhase::Idle,
            text: "暂无进行中的任务".into(),
            done: 0,
            total: 0,
            current_title: None,
            waiting_hint: None,
        }
    }
}

/// Minimal task row for pure projection (wire-agnostic).
#[derive(Debug, Clone)]
pub struct TaskStatusSnap {
    pub title: String,
    /// snake_case task status: pending / running / done / failed / …
    pub status: String,
}

/// Minimal plan-job row for pure projection.
#[derive(Debug, Clone)]
pub struct PlanJobSnap {
    /// planning | planned | confirmed | plan_failed | …
    pub status: String,
    pub task_count: Option<u32>,
}

/// Dual-source resolve (H1-1 priority, nail it):
/// 1. Active (non-terminal) run → **run only**
/// 2. Else terminal run with no job → finished summary
/// 3. Else plan job planning → 规划中
/// 4. planned / await confirm → 已拆成 N 步，等你确认
/// 5. Else finished run if present
/// 6. Idle
pub fn resolve_status_one_liner(
    run: Option<(&str, &[TaskStatusSnap], bool)>,
    job: Option<&PlanJobSnap>,
) -> StatusOneLiner {
    if let Some((run_status, tasks, stall_any)) = run {
        if is_active_run(run_status) {
            return from_run(run_status, tasks, stall_any);
        }
        // Terminal run present: prefer run summary unless we only care about a newer job
        // with no active run (caller passes job when empty live).
        if job.is_none() {
            return from_run(run_status, tasks, stall_any);
        }
        // Both: active job planning/planned wins over historical finished run for "where am I".
        if let Some(j) = job {
            if matches!(j.status.as_str(), "planning" | "planned" | "plan_failed") {
                return from_plan_job(j);
            }
        }
        return from_run(run_status, tasks, stall_any);
    }
    if let Some(j) = job {
        return from_plan_job(j);
    }
    StatusOneLiner::idle()
}

/// Project from run snapshot only.
pub fn from_run(run_status: &str, tasks: &[TaskStatusSnap], stall_any: bool) -> StatusOneLiner {
    let total = tasks.len() as u32;
    let done = tasks.iter().filter(|t| t.status == "done").count() as u32;
    let current = tasks
        .iter()
        .find(|t| matches!(t.status.as_str(), "running" | "starting" | "queued"))
        .map(|t| t.title.clone())
        .filter(|s| !s.trim().is_empty());
    let waiting = tasks
        .iter()
        .filter(|t| matches!(t.status.as_str(), "pending" | "queued"))
        .count();

    let phase = match run_status {
        "paused" => StatusPhase::Paused,
        "completed" => StatusPhase::Completed,
        "failed" => StatusPhase::Failed,
        "aborted" => StatusPhase::Aborted,
        "init" | "validated" | "running" | "starting" | "queued" | "resuming" => {
            StatusPhase::Running
        }
        _ => StatusPhase::Running,
    };

    let text = match phase {
        StatusPhase::Paused => {
            format!("已暂停 · 完成 {done}/{total} 项任务")
        }
        StatusPhase::Completed => {
            format!("本轮状态：**已完成** · 完成 {done}/{total} 项任务")
        }
        StatusPhase::Failed => {
            format!("本轮状态：**失败** · 完成 {done}/{total} 项任务")
        }
        StatusPhase::Aborted => {
            format!("本轮状态：**已中止** · 完成 {done}/{total} 项任务")
        }
        StatusPhase::Running => {
            let mut base = if let Some(ref title) = current {
                let n = tasks
                    .iter()
                    .position(|t| {
                        matches!(t.status.as_str(), "running" | "starting" | "queued")
                            && t.title == *title
                    })
                    .map(|i| i + 1)
                    .unwrap_or(done as usize + 1);
                format!("第 {n} 步在跑：{title} · 完成 {done}/{total}")
            } else if waiting > 0 {
                format!("排队中 · 完成 {done}/{total} 项任务")
            } else if total == 0 {
                "执行中".into()
            } else {
                format!("进行中 · 完成 {done}/{total} 项任务")
            };
            if stall_any {
                base.push_str(" · 有步骤好像卡住了");
            }
            base
        }
        _ => format!("本轮状态 · 完成 {done}/{total} 项任务"),
    };

    let waiting_hint = if waiting > 0 && phase == StatusPhase::Running {
        Some(format!("还有 {waiting} 步在等"))
    } else {
        None
    };

    StatusOneLiner {
        phase,
        text,
        done,
        total,
        current_title: current,
        waiting_hint,
    }
}

/// Project from plan job only.
pub fn from_plan_job(job: &PlanJobSnap) -> StatusOneLiner {
    let n = job.task_count.unwrap_or(0);
    match job.status.as_str() {
        "planning" => StatusOneLiner {
            phase: StatusPhase::Planning,
            text: "规划中，正在拆步骤…".into(),
            done: 0,
            total: n,
            current_title: None,
            waiting_hint: None,
        },
        "planned" | "ready" => StatusOneLiner {
            phase: StatusPhase::AwaitConfirm,
            text: if n > 0 {
                format!("已拆成 {n} 步，等你确认")
            } else {
                "已拆好，等你确认".into()
            },
            done: 0,
            total: n,
            current_title: None,
            waiting_hint: Some("确认后才会开跑".into()),
        },
        "confirmed" => StatusOneLiner {
            phase: StatusPhase::Running,
            text: if n > 0 {
                format!("已确认 {n} 步，准备执行")
            } else {
                "已确认，准备执行".into()
            },
            done: 0,
            total: n,
            current_title: None,
            waiting_hint: None,
        },
        "plan_failed" => StatusOneLiner {
            phase: StatusPhase::Failed,
            text: "规划没成功，可重新拆分".into(),
            done: 0,
            total: n,
            current_title: None,
            waiting_hint: None,
        },
        other => StatusOneLiner {
            phase: StatusPhase::Idle,
            text: format!("计划状态：{other}"),
            done: 0,
            total: n,
            current_title: None,
            waiting_hint: None,
        },
    }
}

fn is_active_run(status: &str) -> bool {
    matches!(
        status,
        "init" | "validated" | "running" | "paused" | "starting" | "queued" | "resuming"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasks(specs: &[(&str, &str)]) -> Vec<TaskStatusSnap> {
        specs
            .iter()
            .map(|(title, st)| TaskStatusSnap {
                title: (*title).into(),
                status: (*st).into(),
            })
            .collect()
    }

    #[test]
    fn job_only_planned() {
        let j = PlanJobSnap {
            status: "planned".into(),
            task_count: Some(6),
        };
        let s = resolve_status_one_liner(None, Some(&j));
        assert_eq!(s.phase, StatusPhase::AwaitConfirm);
        assert!(s.text.contains("6"));
        assert!(s.text.contains("确认"));
    }

    #[test]
    fn job_only_planning() {
        let j = PlanJobSnap {
            status: "planning".into(),
            task_count: None,
        };
        let s = resolve_status_one_liner(None, Some(&j));
        assert_eq!(s.phase, StatusPhase::Planning);
        assert!(s.text.contains("规划"));
    }

    #[test]
    fn run_active_wins_over_job() {
        let ts = tasks(&[("实现登录", "running"), ("写测", "pending")]);
        let j = PlanJobSnap {
            status: "planned".into(),
            task_count: Some(2),
        };
        let s = resolve_status_one_liner(Some(("running", &ts, false)), Some(&j));
        assert_eq!(s.phase, StatusPhase::Running);
        assert!(s.text.contains("实现登录") || s.text.contains("在跑"));
        assert!(!s.text.contains("等你确认"));
    }

    #[test]
    fn finished_run_summary() {
        let ts = tasks(&[("a", "done"), ("b", "done")]);
        let s = resolve_status_one_liner(Some(("completed", &ts, false)), None);
        assert_eq!(s.phase, StatusPhase::Completed);
        assert!(s.text.contains("已完成"));
        assert!(s.text.contains("2/2"));
    }

    #[test]
    fn stall_appends_hint() {
        let ts = tasks(&[("慢步", "running")]);
        let s = from_run("running", &ts, true);
        assert!(s.text.contains("卡住"));
    }

    #[test]
    fn idle_when_empty() {
        let s = resolve_status_one_liner(None, None);
        assert_eq!(s.phase, StatusPhase::Idle);
    }

    #[test]
    fn planned_job_over_historical_completed_run() {
        let ts = tasks(&[("old", "done")]);
        let j = PlanJobSnap {
            status: "planned".into(),
            task_count: Some(4),
        };
        let s = resolve_status_one_liner(Some(("completed", &ts, false)), Some(&j));
        assert_eq!(s.phase, StatusPhase::AwaitConfirm);
        assert!(s.text.contains("4"));
    }
}
