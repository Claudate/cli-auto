//! Runtime collaboration: wait_for conditions integration test.
//!
//! Tests the collaboration bus directly without scheduler integration.

use std::sync::Arc;
use std::time::Duration;

use cco::domain::plan::{WaitCondition, WaitType};
use cco::runtime::collab::{CollabBus, TaskEvent};

#[tokio::test]
async fn wait_for_output_match() {
    let bus = Arc::new(CollabBus::new());
    
    let wait_cond = WaitCondition {
        task_id: "serve".into(),
        condition: WaitType::OutputMatch,
        pattern: Some("Server ready".into()),
    };
    
    // Spawn publisher task
    let bus_clone = bus.clone();
    let publisher = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        bus_clone.publish(TaskEvent::Output {
            task_id: "serve".into(),
            line: "Starting server...".into(),
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        bus_clone.publish(TaskEvent::Output {
            task_id: "serve".into(),
            line: "Server ready at http://localhost:3000".into(),
        });
    });
    
    // Wait for condition
    let result = bus.wait_condition(&wait_cond, Duration::from_secs(2)).await;
    
    assert!(result.is_ok(), "wait_condition should succeed when pattern matches");
    publisher.await.unwrap();
}

#[tokio::test]
async fn wait_for_step_done() {
    let bus = Arc::new(CollabBus::new());
    
    let wait_cond = WaitCondition {
        task_id: "build".into(),
        condition: WaitType::StepDone,
        pattern: Some("compile".into()),
    };
    
    let bus_clone = bus.clone();
    let publisher = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        bus_clone.publish(TaskEvent::Output {
            task_id: "build".into(),
            line: "CCO_STEP start: compile".into(),
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        bus_clone.publish(TaskEvent::Step {
            task_id: "build".into(),
            step: "compile".into(),
            status: "done".into(),
        });
    });
    
    let result = bus.wait_condition(&wait_cond, Duration::from_secs(2)).await;
    
    assert!(result.is_ok(), "wait_condition should succeed on step done");
    publisher.await.unwrap();
}

#[tokio::test]
async fn wait_for_timeout() {
    let bus = Arc::new(CollabBus::new());
    
    let wait_cond = WaitCondition {
        task_id: "nonexistent".into(),
        condition: WaitType::OutputMatch,
        pattern: Some("never happens".into()),
    };
    
    let result = bus.wait_condition(&wait_cond, Duration::from_millis(100)).await;
    
    assert!(result.is_err(), "wait_condition should timeout when condition never met");
    assert!(result.unwrap_err().to_string().contains("timeout"));
}

#[tokio::test]
async fn parse_step_markers() {
    let tests = vec![
        ("CCO_STEP done: compile", Some(("compile".into(), "done".into()))),
        ("CCO_STEP start: test", Some(("test".into(), "start".into()))),
        ("  CCO_STEP done: deploy  ", Some(("deploy".into(), "done".into()))),
        ("Random output", None),
        ("CCO_STEP invalid", None),
    ];
    
    for (input, expected) in tests {
        let result = CollabBus::parse_step_marker(input);
        assert_eq!(result, expected, "parse_step_marker failed for: {}", input);
    }
}
