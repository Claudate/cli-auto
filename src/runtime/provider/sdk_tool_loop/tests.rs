use super::*;
use crate::plan::TaskIR;
use crate::runtime::provider::{SdkProvider, TaskStatus, WorkerPort, WorkerStatus};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

/// Scripted multi-call HTTP mock: each post_json pops next response.
struct ScriptedClient {
    responses: Mutex<Vec<(u16, String)>>,
    call_count: AtomicUsize,
    last_bodies: Mutex<Vec<Value>>,
}

impl ScriptedClient {
    fn new(responses: Vec<(u16, String)>) -> Self {
        Self {
            responses: Mutex::new(responses),
            call_count: AtomicUsize::new(0),
            last_bodies: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl MessagesHttpClient for ScriptedClient {
    async fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        body: Value,
    ) -> Result<(u16, String)> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.last_bodies.lock().unwrap().push(body);
        let mut guard = self.responses.lock().unwrap();
        if guard.is_empty() {
            bail!("scripted client: no more responses");
        }
        Ok(guard.remove(0))
    }
}

fn sample_task(id: &str, prompt: &str) -> TaskIR {
    TaskIR {
        id: id.into(),
        title: id.into(),
        depends_on: vec![],
        group: None,
        provider: "sdk".into(),
        mode: "print".into(),
        prompt: prompt.into(),
        acceptance: None,
        timeout_secs: None,
        worktree: None,
        provider_opts: serde_json::json!({}),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
    }
}

fn ctx(dir: &std::path::Path) -> StartCtx {
    let task_dir = dir.join("tasks").join("t1");
    std::fs::create_dir_all(&task_dir).unwrap();
    StartCtx {
        run_id: "run-sdk-s2".into(),
        project_root: dir.to_path_buf(),
        work_dir: dir.to_path_buf(),
        task_dir,
        env_extra: vec![],
    }
}

fn tool_use_response(id: &str, name: &str, input: Value) -> String {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "stop_reason": "tool_use",
        "content": [{
            "type": "tool_use",
            "id": "toolu_1",
            "name": name,
            "input": input,
        }],
        "usage": { "input_tokens": 5, "output_tokens": 8 },
    })
    .to_string()
}

fn end_turn_response(id: &str, text: &str) -> String {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "stop_reason": "end_turn",
        "content": [{ "type": "text", "text": text }],
        "usage": { "input_tokens": 12, "output_tokens": 20 },
    })
    .to_string()
}

#[test]
fn is_tools_bin_aliases() {
    assert!(is_tools_bin("tools"));
    assert!(is_tools_bin("tool_loop"));
    assert!(is_tools_bin("tool-loop"));
    assert!(is_tools_bin("agent"));
    assert!(is_tools_bin("TOOLS"));
    assert!(!is_tools_bin("messages"));
    assert!(!is_tools_bin("inline"));
}

#[test]
fn resolve_under_blocks_escape() {
    let root = PathBuf::from("/tmp/work");
    assert!(resolve_under(&root, "a/b.txt")
        .unwrap()
        .ends_with("a/b.txt"));
    assert!(resolve_under(&root, "../secret").is_err());
    assert!(resolve_under(&root, "/etc/passwd").is_err());
    assert!(resolve_under(&root, "").is_err());
}

#[tokio::test]
async fn tool_loop_read_then_end_via_mock() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "hello-tools").unwrap();

    let client = ScriptedClient::new(vec![
        (
            200,
            tool_use_response("msg_1", "read_file", json!({ "path": "note.txt" })),
        ),
        (200, end_turn_response("msg_2", "file says hello-tools")),
    ]);
    let backend = AnthropicToolLoopBackend::new(
        client,
        "test-key",
        "claude-test",
        "https://api.example.test",
        1024,
        8,
    );
    backend.preflight().await.unwrap();

    let provider = SdkProvider::with_backend(Arc::new(backend));
    let handle = provider
        .start(&sample_task("t1", "read note.txt"), &ctx(dir.path()))
        .await
        .unwrap();
    assert!(matches!(
        provider.poll(&handle).await.unwrap(),
        WorkerStatus::Done
    ));
    let result = provider.collect(&handle).await.unwrap();
    assert_eq!(result.status, TaskStatus::Done);
    assert_eq!(result.exit_code, Some(0));
    let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
    assert!(stdout.contains("CCO_DONE"), "stdout: {stdout}");
    assert!(stdout.contains("\"backend\":\"tools\""));
    assert!(stdout.contains("hello-tools") || stdout.contains("file says hello-tools"));
    assert!(stdout.contains("\"type\":\"tool\""));
    assert_eq!(result.session_id.as_deref(), Some("msg_1"));

    let meta = std::fs::read_to_string(&handle.meta_path).unwrap();
    assert!(meta.contains("tools"), "meta: {meta}");
}

#[tokio::test]
async fn tool_loop_write_file_under_work_dir() {
    let dir = tempdir().unwrap();
    let client = ScriptedClient::new(vec![
        (
            200,
            tool_use_response(
                "msg_w1",
                "write_file",
                json!({ "path": "out/hi.txt", "content": "written-by-s2" }),
            ),
        ),
        (200, end_turn_response("msg_w2", "wrote it")),
    ]);
    let backend =
        AnthropicToolLoopBackend::new(client, "k", "m", "https://api.example.test", 256, 4);
    let provider = SdkProvider::with_backend(Arc::new(backend));
    let handle = provider
        .start(&sample_task("w1", "write"), &ctx(dir.path()))
        .await
        .unwrap();
    assert_eq!(
        provider.collect(&handle).await.unwrap().status,
        TaskStatus::Done
    );
    let written = std::fs::read_to_string(dir.path().join("out/hi.txt")).unwrap();
    assert_eq!(written, "written-by-s2");
}

#[tokio::test]
async fn tool_loop_max_rounds_fails() {
    let dir = tempdir().unwrap();
    let always_tool = tool_use_response("msg_loop", "list_dir", json!({ "path": "." }));
    let client = ScriptedClient::new(vec![(200, always_tool.clone()), (200, always_tool)]);
    let backend =
        AnthropicToolLoopBackend::new(client, "k", "m", "https://api.example.test", 256, 2);
    let provider = SdkProvider::with_backend(Arc::new(backend));
    let handle = provider
        .start(&sample_task("loop", "spin"), &ctx(dir.path()))
        .await
        .unwrap();
    assert!(matches!(
        provider.poll(&handle).await.unwrap(),
        WorkerStatus::Failed
    ));
    let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
    assert!(stdout.contains("max rounds"), "stdout: {stdout}");
}

#[tokio::test]
async fn tool_loop_preflight_requires_key() {
    let backend =
        AnthropicToolLoopBackend::new(ScriptedClient::new(vec![]), "", "m", "https://api.example.test", 64, 2);
    let err = backend.preflight().await.unwrap_err().to_string();
    assert!(err.contains("API key"), "err: {err}");
}

#[tokio::test]
async fn tool_loop_request_includes_tools() {
    let dir = tempdir().unwrap();
    let client = Arc::new(ScriptedClient::new(vec![(
        200,
        end_turn_response("msg_only", "no tools needed"),
    )]));

    struct Shared(Arc<ScriptedClient>);
    #[async_trait]
    impl MessagesHttpClient for Shared {
        async fn post_json(
            &self,
            url: &str,
            headers: &[(&str, String)],
            body: Value,
        ) -> Result<(u16, String)> {
            self.0.post_json(url, headers, body).await
        }
    }

    let backend = AnthropicToolLoopBackend::new(
        Shared(Arc::clone(&client)),
        "k",
        "m",
        "https://api.example.test",
        128,
        3,
    );
    let provider = SdkProvider::with_backend(Arc::new(backend));
    let _ = provider
        .start(&sample_task("t", "hi"), &ctx(dir.path()))
        .await
        .unwrap();
    let bodies = client.last_bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1);
    let tools = bodies[0]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_dir"));
    assert!(names.contains(&"write_file"));
}
