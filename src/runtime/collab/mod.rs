//! Runtime collaboration bus for agent inter-task communication.
//!
//! [INPUT]: Task output events · WaitCondition
//! [OUTPUT]: Condition satisfied notifications · real-time output subscription
//! [POS]: runtime/collab — event bus for task coordination
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
//!
//! ## Design
//! Uses tokio::sync::broadcast for in-memory event distribution.
//! Each task gets its own channel; subscribers receive clones of events.
//! A bounded per-task history buffer lets late subscribers (e.g. a task
//! spawned after its dependency already printed the awaited line) match
//! conditions against past events via [`CollabBus::condition_met`].

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use tokio::sync::broadcast;
use tracing::debug;

use crate::domain::plan::{WaitCondition, WaitType};
use crate::runtime::provider::TaskStatus;

/// Task event published to the collaboration bus
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// Output line from task (stdout)
    Output { task_id: String, line: String },
    /// Step marker parsed from output (CCO_STEP done:name or CCO_STEP start:name)
    Step {
        task_id: String,
        step: String,
        status: String,
    },
    /// Task status changed
    StatusChange { task_id: String, status: TaskStatus },
}

/// Collaboration bus for runtime task coordination
#[derive(Clone)]
pub struct CollabBus {
    /// Broadcast channels per task (task_id -> sender)
    channels: Arc<Mutex<HashMap<String, broadcast::Sender<TaskEvent>>>>,
    /// Bounded event history per task, for late subscribers (task_id -> events)
    history: Arc<Mutex<HashMap<String, VecDeque<TaskEvent>>>>,
    /// Channel + history capacity (default 1000 events)
    capacity: usize,
}

impl CollabBus {
    /// Create a new collaboration bus with default capacity
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    /// Create with specific channel capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
            history: Arc::new(Mutex::new(HashMap::new())),
            capacity,
        }
    }

    /// Publish an event to the bus
    pub fn publish(&self, event: TaskEvent) {
        let task_id = match &event {
            TaskEvent::Output { task_id, .. } => task_id,
            TaskEvent::Step { task_id, .. } => task_id,
            TaskEvent::StatusChange { task_id, .. } => task_id,
        };

        {
            let mut history = self.history.lock().unwrap();
            let buf = history.entry(task_id.clone()).or_default();
            if buf.len() >= self.capacity {
                buf.pop_front();
            }
            buf.push_back(event.clone());
        }

        let mut channels = self.channels.lock().unwrap();
        let tx = channels
            .entry(task_id.clone())
            .or_insert_with(|| broadcast::channel(self.capacity).0);

        // Ignore send errors (no receivers is fine)
        let _ = tx.send(event);
    }

    /// Check a condition against past events (non-blocking).
    ///
    /// The scheduler uses this instead of [`Self::wait_condition`] because the
    /// same loop that would await the condition is the one publishing events —
    /// blocking inline would deadlock until timeout.
    pub fn condition_met(&self, cond: &WaitCondition) -> bool {
        let history = self.history.lock().unwrap();
        history
            .get(&cond.task_id)
            .map(|buf| buf.iter().any(|ev| self.check_condition(cond, ev)))
            .unwrap_or(false)
    }

    /// Subscribe to a task's events
    pub fn subscribe(&self, task_id: &str) -> broadcast::Receiver<TaskEvent> {
        let mut channels = self.channels.lock().unwrap();
        let tx = channels
            .entry(task_id.to_string())
            .or_insert_with(|| broadcast::channel(self.capacity).0);
        tx.subscribe()
    }

    /// Wait for a condition to be satisfied (with timeout)
    pub async fn wait_condition(
        &self,
        cond: &WaitCondition,
        timeout: Duration,
    ) -> Result<()> {
        // Subscribe before replaying history so no event can slip between.
        let mut rx = self.subscribe(&cond.task_id);
        if self.condition_met(cond) {
            return Ok(());
        }

        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                bail!(
                    "wait condition timeout: task {} condition {:?}",
                    cond.task_id,
                    cond.condition
                );
            }

            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(event)) => {
                    if self.check_condition(cond, &event) {
                        debug!(
                            task = %cond.task_id,
                            condition = ?cond.condition,
                            "condition satisfied"
                        );
                        return Ok(());
                    }
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    bail!("task {} channel closed before condition met", cond.task_id);
                }
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                    // Channel lagged, continue (we may have missed the event, but
                    // subsequent events might still satisfy the condition)
                    continue;
                }
                Err(_) => {
                    bail!(
                        "wait condition timeout: task {} condition {:?}",
                        cond.task_id,
                        cond.condition
                    );
                }
            }
        }
    }

    /// Check if an event satisfies a wait condition
    fn check_condition(&self, cond: &WaitCondition, event: &TaskEvent) -> bool {
        match (&cond.condition, event) {
            (WaitType::OutputMatch, TaskEvent::Output { line, .. }) => {
                if let Some(pattern) = &cond.pattern {
                    line.contains(pattern)
                } else {
                    false
                }
            }
            (WaitType::StepDone, TaskEvent::Step { step, status, .. }) => {
                status == "done"
                    && cond
                        .pattern
                        .as_ref()
                        .map(|p| step.contains(p))
                        .unwrap_or(true)
            }
            (WaitType::Complete, TaskEvent::StatusChange { status, .. }) => status.is_terminal(),
            _ => false,
        }
    }

    /// Parse step markers from output line
    pub fn parse_step_marker(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("CCO_STEP done:") {
            let step = rest.trim().to_string();
            return Some((step, "done".to_string()));
        }
        if let Some(rest) = trimmed.strip_prefix("CCO_STEP start:") {
            let step = rest.trim().to_string();
            return Some((step, "start".to_string()));
        }
        None
    }
}

impl Default for CollabBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn output_match_condition() {
        let bus = CollabBus::new();
        let cond = WaitCondition {
            task_id: "serve".to_string(),
            condition: WaitType::OutputMatch,
            pattern: Some("Server ready".to_string()),
        };

        // Spawn waiter
        let bus_clone = bus.clone();
        let cond_clone = cond.clone();
        let waiter = tokio::spawn(async move {
            bus_clone
                .wait_condition(&cond_clone, Duration::from_secs(2))
                .await
        });

        // Give waiter time to subscribe
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Publish non-matching event
        bus.publish(TaskEvent::Output {
            task_id: "serve".to_string(),
            line: "Starting...".to_string(),
        });

        // Publish matching event
        bus.publish(TaskEvent::Output {
            task_id: "serve".to_string(),
            line: "Server ready at http://localhost:3000".to_string(),
        });

        // Wait should complete
        assert!(waiter.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn step_done_condition() {
        let bus = CollabBus::new();
        let cond = WaitCondition {
            task_id: "build".to_string(),
            condition: WaitType::StepDone,
            pattern: Some("compile".to_string()),
        };

        let bus_clone = bus.clone();
        let cond_clone = cond.clone();
        let waiter = tokio::spawn(async move {
            bus_clone
                .wait_condition(&cond_clone, Duration::from_secs(2))
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        bus.publish(TaskEvent::Step {
            task_id: "build".to_string(),
            step: "compile-frontend".to_string(),
            status: "done".to_string(),
        });

        assert!(waiter.await.unwrap().is_ok());
    }

    #[test]
    fn parse_step_markers() {
        assert_eq!(
            CollabBus::parse_step_marker("CCO_STEP done: compile"),
            Some(("compile".to_string(), "done".to_string()))
        );
        assert_eq!(
            CollabBus::parse_step_marker("CCO_STEP start: tests"),
            Some(("tests".to_string(), "start".to_string()))
        );
        assert_eq!(
            CollabBus::parse_step_marker("  CCO_STEP done:build-assets  "),
            Some(("build-assets".to_string(), "done".to_string()))
        );
        assert_eq!(CollabBus::parse_step_marker("normal output"), None);
    }

    #[tokio::test]
    async fn late_subscriber_matches_history() {
        let bus = CollabBus::new();
        let cond = WaitCondition {
            task_id: "serve".to_string(),
            condition: WaitType::OutputMatch,
            pattern: Some("Server ready".to_string()),
        };

        // Publish BEFORE anyone subscribes/waits — must still match via history.
        bus.publish(TaskEvent::Output {
            task_id: "serve".to_string(),
            line: "Server ready at http://localhost:3000".to_string(),
        });

        assert!(bus.condition_met(&cond), "condition_met must scan history");
        assert!(
            bus.wait_condition(&cond, Duration::from_millis(100))
                .await
                .is_ok(),
            "wait_condition must replay history for late subscribers"
        );
    }

    #[tokio::test]
    async fn timeout_when_condition_not_met() {
        let bus = CollabBus::new();
        let cond = WaitCondition {
            task_id: "never".to_string(),
            condition: WaitType::OutputMatch,
            pattern: Some("never appears".to_string()),
        };

        let result = bus
            .wait_condition(&cond, Duration::from_millis(100))
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("timeout"));
    }
}
