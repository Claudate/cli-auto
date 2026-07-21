//! Cwd-scoped tool defs + execution for S2 tool loop.

use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

pub const MAX_READ_BYTES: u64 = 256 * 1024;
pub const MAX_LIST_ENTRIES: usize = 200;
pub const MAX_TOOL_RESULT_CHARS: usize = 32_000;

pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

pub fn tool_defs() -> Value {
    json!([
        {
            "name": "read_file",
            "description": "Read a UTF-8 text file under the task work directory (relative path only).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative path under work_dir" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "list_dir",
            "description": "List entries in a directory under the task work directory.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative directory path (default \".\")"
                    }
                }
            }
        },
        {
            "name": "write_file",
            "description": "Write UTF-8 text to a file under the task work directory (creates parents).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
    ])
}

pub fn extract_tool_uses(content: &Value) -> Vec<ToolUse> {
    let Some(arr) = content.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }
        let id = block
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = block
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
        out.push(ToolUse { id, name, input });
    }
    out
}

pub fn extract_text_blocks(content: &Value) -> Option<String> {
    let arr = content.as_array()?;
    let mut parts = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

pub fn run_tool(name: &str, input: &Value, work_dir: &Path) -> Result<String> {
    match name {
        "read_file" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let full = resolve_under(work_dir, path)?;
            let meta =
                std::fs::metadata(&full).map_err(|e| anyhow!("stat {}: {e}", full.display()))?;
            if !meta.is_file() {
                bail!("not a file: {path}");
            }
            if meta.len() > MAX_READ_BYTES {
                bail!(
                    "file too large ({} bytes; max {MAX_READ_BYTES})",
                    meta.len()
                );
            }
            let data = std::fs::read_to_string(&full)
                .map_err(|e| anyhow!("read {}: {e}", full.display()))?;
            Ok(data)
        }
        "list_dir" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .trim();
            let path = if path.is_empty() { "." } else { path };
            let full = resolve_under(work_dir, path)?;
            let rd =
                std::fs::read_dir(&full).map_err(|e| anyhow!("list {}: {e}", full.display()))?;
            let mut names = Vec::new();
            for ent in rd.flatten() {
                let name = ent.file_name().to_string_lossy().to_string();
                let suffix = if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "/"
                } else {
                    ""
                };
                names.push(format!("{name}{suffix}"));
                if names.len() >= MAX_LIST_ENTRIES {
                    names.push("…(truncated)".into());
                    break;
                }
            }
            names.sort();
            Ok(names.join("\n"))
        }
        "write_file" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let full = resolve_under(work_dir, path)?;
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("mkdir {}: {e}", parent.display()))?;
            }
            std::fs::write(&full, content)
                .map_err(|e| anyhow!("write {}: {e}", full.display()))?;
            Ok(format!("wrote {} bytes to {path}", content.len()))
        }
        other => bail!("unknown tool: {other}"),
    }
}

/// Resolve `user` as a relative path strictly under `root` (no absolute, no `..` escape).
pub fn resolve_under(root: &Path, user: &str) -> Result<PathBuf> {
    let user = user.trim();
    if user.is_empty() {
        bail!("empty path");
    }
    let rel = Path::new(user);
    if rel.is_absolute() {
        bail!("absolute path not allowed");
    }
    let mut acc = PathBuf::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => acc.push(s),
            Component::CurDir => {}
            Component::ParentDir => {
                if !acc.pop() {
                    bail!("path escapes work_dir");
                }
            }
            Component::RootDir | Component::Prefix(_) => bail!("invalid path component"),
        }
    }
    Ok(root.join(acc))
}
