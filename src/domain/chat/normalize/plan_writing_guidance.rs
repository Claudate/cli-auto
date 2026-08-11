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
pub const FILE_UI_DELIVERY_RECIPES: &str = "ui-delivery-recipes.md";
pub const FILE_UI_LAYOUT: &str = "ui-layout-systems.md";
pub const FILE_UI_COLOR_SYSTEMS: &str = "ui-color-systems.md";
pub const FILE_UI_TYPOGRAPHY: &str = "ui-typography-systems.md";
pub const FILE_UI_MOTION: &str = "ui-motion-effects.md";
pub const FILE_UI_COPY: &str = "ui-copy-systems.md";
pub const FILE_BACKEND_ARCHITECTURE: &str = "backend-architecture.md";
pub const FILE_CHAT_VISUAL_REVIEW: &str = "chat-visual-review.md";

/// Env: absolute or relative dir containing the markdown files above.
pub const ENV_RUNTIME_PROMPTS_DIR: &str = "CCO_RUNTIME_PROMPTS_DIR";

// Compile-time embed = packaging fallback when no disk copy exists (still "from document").
const EMBED_CHAT: &str = include_str!("../../docs/runtime-prompts/chat-plan-writing.md");
const EMBED_SPLIT: &str = include_str!("../../docs/runtime-prompts/split-agent-delivery.md");
const EMBED_PLANNER: &str =
    include_str!("../../docs/runtime-prompts/planner-greenfield-stack.md");
const EMBED_UI_RECIPES: &str = include_str!("../../docs/runtime-prompts/ui-delivery-recipes.md");
const EMBED_UI_LAYOUT: &str = include_str!("../../docs/runtime-prompts/ui-layout-systems.md");
const EMBED_UI_COLOR: &str = include_str!("../../docs/runtime-prompts/ui-color-systems.md");
const EMBED_UI_TYPE: &str = include_str!("../../docs/runtime-prompts/ui-typography-systems.md");
const EMBED_UI_MOTION: &str = include_str!("../../docs/runtime-prompts/ui-motion-effects.md");
const EMBED_UI_COPY: &str = include_str!("../../docs/runtime-prompts/ui-copy-systems.md");
const EMBED_BACKEND: &str = include_str!("../../docs/runtime-prompts/backend-architecture.md");
const EMBED_CHAT_VISUAL: &str = include_str!("../../docs/runtime-prompts/chat-visual-review.md");

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

/// End-to-end effect recipes (layout+color+type+motion+images+backend).
pub fn ui_delivery_recipes_guidance() -> &'static str {
    load_cached(FILE_UI_DELIVERY_RECIPES, EMBED_UI_RECIPES)
}

/// Site type + section order + information architecture (append to chat / split).
pub fn ui_layout_systems_guidance() -> &'static str {
    load_cached(FILE_UI_LAYOUT, EMBED_UI_LAYOUT)
}

/// Color kits + CSS token discipline (append to chat / split system prompts).
pub fn ui_color_systems_guidance() -> &'static str {
    load_cached(FILE_UI_COLOR_SYSTEMS, EMBED_UI_COLOR)
}

/// Font kits display/body/ui aligned with color kits (append to chat / split).
pub fn ui_typography_systems_guidance() -> &'static str {
    load_cached(FILE_UI_TYPOGRAPHY, EMBED_UI_TYPE)
}

/// Motion tiers + open-source effect whitelist (append to chat / split).
pub fn ui_motion_effects_guidance() -> &'static str {
    load_cached(FILE_UI_MOTION, EMBED_UI_MOTION)
}

/// Product UI copy: website + app/software microcopy (append to chat / split).
pub fn ui_copy_systems_guidance() -> &'static str {
    load_cached(FILE_UI_COPY, EMBED_UI_COPY)
}

/// Delivery depth A–D + language/framework + MVC/MVVM/DDD (append to chat / split).
pub fn backend_architecture_guidance() -> &'static str {
    load_cached(FILE_BACKEND_ARCHITECTURE, EMBED_BACKEND)
}

/// Chat visual QA: screenshot → analyze → embed `![](path)` → optimize advice.
pub fn chat_visual_review_guidance() -> &'static str {
    load_cached(FILE_CHAT_VISUAL_REVIEW, EMBED_CHAT_VISUAL)
}

fn load_cached(file_name: &'static str, embedded: &'static str) -> &'static str {
    match file_name {
        FILE_CHAT_PLAN_WRITING => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_SPLIT_AGENT_DELIVERY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_PLANNER_GREENFIELD => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_DELIVERY_RECIPES => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_LAYOUT => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_COLOR_SYSTEMS => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_TYPOGRAPHY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_MOTION => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_UI_COPY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_BACKEND_ARCHITECTURE => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
        }
        FILE_CHAT_VISUAL_REVIEW => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded))
                .as_str()
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
include!("plan_writing_guidance_tests.rs");
