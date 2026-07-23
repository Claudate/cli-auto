//! Load product guidance from **Markdown docs** (not hardcoded essays in Rust).
//!
//! [INPUT]: optional disk paths · compile-time embed of `docs/runtime-prompts/*`
//! [OUTPUT]: prompt fragments for chat / split-agent / planner
//! [POS]: domain/chat — 软件内底层知识；真源见 `docs/runtime-prompts/README.md`
//! [PROTOCOL]: 增删文件名须同步 docs/runtime-prompts 与本文件常量

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Filenames under a `runtime-prompts` directory.
pub const FILE_CHAT_PLAN_WRITING: &str = "chat-plan-writing.md";
pub const FILE_SPLIT_AGENT_DELIVERY: &str = "split-agent-delivery.md";
pub const FILE_PLANNER_GREENFIELD: &str = "planner-greenfield-stack.md";

/// Env: absolute or relative dir containing the markdown files above.
pub const ENV_RUNTIME_PROMPTS_DIR: &str = "CCO_RUNTIME_PROMPTS_DIR";

// Compile-time embed = packaging fallback when no disk copy exists (still "from document").
const EMBED_CHAT: &str = include_str!("../../../docs/runtime-prompts/chat-plan-writing.md");
const EMBED_SPLIT: &str = include_str!("../../../docs/runtime-prompts/split-agent-delivery.md");
const EMBED_PLANNER: &str =
    include_str!("../../../docs/runtime-prompts/planner-greenfield-stack.md");

/// Compact architect + frontend co-plan rules for the desktop chat system prompt.
pub fn chat_plan_writing_guidance() -> &'static str {
    load_cached(FILE_CHAT_PLAN_WRITING, EMBED_CHAT)
}

/// Extra rules for split-agent system prompt (implement body quality).
pub fn split_agent_delivery_guidance() -> &'static str {
    load_cached(FILE_SPLIT_AGENT_DELIVERY, EMBED_SPLIT)
}

/// Extra greenfield planner mode blurb (legacy PlanIR planner).
pub fn planner_greenfield_stack_blurb() -> &'static str {
    load_cached(FILE_PLANNER_GREENFIELD, EMBED_PLANNER)
}

fn load_cached(file_name: &'static str, embedded: &'static str) -> &'static str {
    match file_name {
        FILE_CHAT_PLAN_WRITING => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_SPLIT_AGENT_DELIVERY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_PLANNER_GREENFIELD => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        _ => embedded,
    }
}

fn resolve_text(file_name: &str, embedded: &str) -> String {
    for dir in prompt_search_dirs() {
        let path = dir.join(file_name);
        if let Ok(text) = std::fs::read_to_string(&path) {
            let t = text.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    embedded.trim().to_string()
}

/// Ordered directories to probe for `*.md` (first hit wins).
pub fn prompt_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(raw) = std::env::var(ENV_RUNTIME_PROMPTS_DIR) {
        let p = PathBuf::from(raw.trim());
        if !p.as_os_str().is_empty() {
            dirs.push(p);
        }
    }

    dirs.push(
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cco")
            .join("runtime-prompts"),
    );

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = walk_up_for_runtime_prompts(&cwd) {
            dirs.push(found);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("runtime-prompts"));
            // macOS .app: Contents/MacOS/cco → Contents/Resources/runtime-prompts
            dirs.push(parent.join("../Resources/runtime-prompts"));
        }
    }

    dirs
}

fn walk_up_for_runtime_prompts(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    for _ in 0..10 {
        let candidate = cur.join("docs").join("runtime-prompts");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_chat_covers_overseas_and_icons() {
        let g = chat_plan_writing_guidance();
        assert!(g.contains("出海"), "got: {}", &g[..g.len().min(200)]);
        assert!(g.contains("静态") || g.contains("Astro"));
        assert!(g.contains("开源线标") || g.contains("Lucide"));
        assert!(g.contains("建议技术"));
    }

    #[test]
    fn split_and_planner_blurbs_nonempty() {
        assert!(split_agent_delivery_guidance().contains("开源线标"));
        assert!(planner_greenfield_stack_blurb().contains("静态"));
    }

    #[test]
    fn search_dirs_include_home_cco() {
        let dirs = prompt_search_dirs();
        assert!(
            dirs.iter().any(|d| d.ends_with("runtime-prompts")),
            "dirs={dirs:?}"
        );
    }
}
