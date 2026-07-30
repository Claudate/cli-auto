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
const EMBED_CHAT: &str = include_str!("../../../docs/runtime-prompts/chat-plan-writing.md");
const EMBED_SPLIT: &str = include_str!("../../../docs/runtime-prompts/split-agent-delivery.md");
const EMBED_PLANNER: &str =
    include_str!("../../../docs/runtime-prompts/planner-greenfield-stack.md");
const EMBED_UI_RECIPES: &str = include_str!("../../../docs/runtime-prompts/ui-delivery-recipes.md");
const EMBED_UI_LAYOUT: &str = include_str!("../../../docs/runtime-prompts/ui-layout-systems.md");
const EMBED_UI_COLOR: &str = include_str!("../../../docs/runtime-prompts/ui-color-systems.md");
const EMBED_UI_TYPE: &str = include_str!("../../../docs/runtime-prompts/ui-typography-systems.md");
const EMBED_UI_MOTION: &str = include_str!("../../../docs/runtime-prompts/ui-motion-effects.md");
const EMBED_UI_COPY: &str = include_str!("../../../docs/runtime-prompts/ui-copy-systems.md");
const EMBED_BACKEND: &str = include_str!("../../../docs/runtime-prompts/backend-architecture.md");
const EMBED_CHAT_VISUAL: &str = include_str!("../../../docs/runtime-prompts/chat-visual-review.md");

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
        FILE_UI_DELIVERY_RECIPES => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_UI_LAYOUT => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_UI_COLOR_SYSTEMS => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_UI_TYPOGRAPHY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_UI_MOTION => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_UI_COPY => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_BACKEND_ARCHITECTURE => {
            static CELL: OnceLock<String> = OnceLock::new();
            CELL.get_or_init(|| resolve_text(file_name, embedded)).as_str()
        }
        FILE_CHAT_VISUAL_REVIEW => {
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
        assert!(
            g.contains("配方") || g.contains("ui-delivery-recipes") || g.contains("R-overseas"),
            "recipes pointer"
        );
        assert!(
            g.contains("站点类型") || g.contains("marketing") || g.contains("layout"),
            "layout pointer"
        );
        assert!(
            g.contains("色系") || g.contains("western-saas"),
            "color kit pointer"
        );
        assert!(
            g.contains("字体") || g.contains("typography") || g.contains("文楷"),
            "typography pointer"
        );
        assert!(
            g.contains("动效") || g.contains("motion") || g.contains("light"),
            "motion pointer"
        );
        assert!(
            g.contains("文案") || g.contains("ui-copy") || g.contains("主 CTA"),
            "copy pointer"
        );
        assert!(
            g.contains("交付深度") || g.contains("backend-architecture"),
            "backend depth pointer"
        );
        assert!(
            g.contains("site-floor") || g.contains("RECIPE-MAP"),
            "premium site-floor starter pointer"
        );
    }

    /// Clarify-phase strategy lives in chat-plan-writing.md (t2 prompt true source).
    #[test]
    fn embedded_chat_covers_clarify_strategy() {
        let g = chat_plan_writing_guidance();
        // Three on-ramps
        assert!(g.contains("想清楚再说"), "think_first label");
        assert!(g.contains("从想法到计划"), "idea_to_plan default");
        assert!(
            g.contains("已想清，直接写计划") || g.contains("plan_only") || g.contains("直接写计划"),
            "plan_only label"
        );
        // Follow-up: ≤5 + A/B/C + 你定 → 假设
        assert!(
            g.contains("≤5") || g.contains("<=5") || g.contains("不超过 5") || g.contains("至多 5"),
            "max 5 questions"
        );
        assert!(g.contains("A/B/C") || g.contains("A. ") || g.contains("A．"), "A/B/C options");
        assert!(
            g.contains("你定") || g.contains("直接出计划"),
            "skip phrase"
        );
        assert!(g.contains("假设"), "must write assumptions on skip");
        // Brief fields
        for needle in ["问题", "给谁", "不做", "验收", "V1"] {
            assert!(g.contains(needle), "Brief field missing: {needle}");
        }
        assert!(
            g.contains("得") && g.contains("失") || g.contains("得 / 失") || g.contains("会失去什么"),
            "gain/loss"
        );
        assert!(g.contains("未决"), "open items");
        // Evidence light tags only
        assert!(g.contains("用户原话"), "evidence: user quote");
        assert!(g.contains("自用痛点"), "evidence: self pain");
        assert!(g.contains("竞品缺口"), "evidence: competitor gap");
        // Plan min chapters + V1 gate
        assert!(
            g.contains("非目标") || g.contains("不做"),
            "non-goals chapter"
        );
        assert!(g.contains("会失去什么") || g.contains("得 / 失"), "loss chapter");
        assert!(g.contains("风险"), "risks chapter");
        assert!(
            g.contains("V2") || g.contains("Later"),
            "V2/Later fold section"
        );
        assert!(
            g.contains("默认") && (g.contains("V1") || g.contains("任务大纲")),
            "V1 default for task outline"
        );
        // Guardrails
        assert!(
            g.contains("VERDICT") || g.contains("run_id"),
            "forbid internal ids as first line"
        );
        assert!(
            g.contains("confirm_start") || g.contains("spawn"),
            "forbid spawn/confirm in chat path"
        );
        assert!(
            !g.contains("Crazy 8") || g.contains("不做") || g.contains("不整包"),
            "boundary mention ok if negative"
        );
    }

    /// W1-5: iterate-clarity + persona lexicon + zero internal mode codes in prompts.
    #[test]
    fn embedded_chat_covers_iterate_and_lexicon() {
        let g = chat_plan_writing_guidance();
        assert!(
            g.contains("当前理解") || g.contains("边聊"),
            "iterate / understanding"
        );
        assert!(
            g.contains("按我说的改")
                || g.contains("按反馈改")
                || g.contains("换个方向"),
            "feedback revise path"
        );
        assert!(
            g.contains("矛盾") || g.contains("冲突"),
            "contradiction alignment"
        );
        assert!(
            g.contains("上架") || g.contains("存档") || g.contains("本波"),
            "scene-specific done-when lexicon"
        );
        assert!(
            g.contains("零") && (g.contains("代号") || g.contains("P1")),
            "forbid teaching internal mode codes"
        );
        // Must not present P-codes as user-facing primary vocabulary positively
        assert!(
            g.contains("禁止") || g.contains("零内部"),
            "negative framing for internal codes"
        );
        // W2-3 multi-plan cutting (bundle) without teaching L/H
        assert!(
            g.contains("索引") || g.contains("多执行计划") || g.contains("多份"),
            "multi-plan / index guidance"
        );
        assert!(
            g.contains("粘成") || g.contains("超长"),
            "forbid glue-into-one long md"
        );
    }

    #[test]
    fn ui_copy_covers_product_ui_strings() {
        let g = ui_copy_systems_guidance();
        assert!(g.contains("主 CTA") || g.contains("动词"));
        assert!(g.contains("空态") || g.contains("错误"));
        assert!(g.contains("App") || g.contains("软件") || g.contains("按钮"));
        assert!(g.contains("Lorem") || g.contains("占位") || g.contains("TODO"));
        assert!(g.contains("人话") || g.contains("帮人办事"));
    }

    #[test]
    fn chat_visual_review_requires_embed_and_honest_shots() {
        let g = chat_visual_review_guidance();
        assert!(g.contains("![") || g.contains("![]"), "require markdown image embed");
        assert!(
            g.contains("截图") || g.contains("screenshot"),
            "screenshot flow"
        );
        assert!(
            g.contains("优化") || g.contains("建议"),
            "optimization advice"
        );
        assert!(
            g.contains("禁止") && (g.contains("编") || g.contains("空想") || g.contains("没截图")),
            "forbid inventing visuals"
        );
    }

    #[test]
    fn split_and_planner_blurbs_nonempty() {
        assert!(split_agent_delivery_guidance().contains("开源线标"));
        assert!(
            split_agent_delivery_guidance().contains("配方")
                || split_agent_delivery_guidance().contains("recipes")
        );
        assert!(
            split_agent_delivery_guidance().contains("信息结构")
                || split_agent_delivery_guidance().contains("站点类型")
                || split_agent_delivery_guidance().contains("排版")
        );
        assert!(
            split_agent_delivery_guidance().contains("占位图")
                || split_agent_delivery_guidance().contains("placehold")
        );
        assert!(planner_greenfield_stack_blurb().contains("静态"));
        assert!(
            planner_greenfield_stack_blurb().contains("配方")
                || planner_greenfield_stack_blurb().contains("结构")
        );
        assert!(
            planner_greenfield_stack_blurb().contains("site-floor"),
            "greenfield must point at site-floor scaffold"
        );
        assert!(
            split_agent_delivery_guidance().contains("site-floor")
                || split_agent_delivery_guidance().contains("RECIPE-MAP"),
            "split delivery must mention site-floor"
        );
        let split = split_agent_delivery_guidance();
        assert!(
            split.contains("上架")
                || split.contains("验收词")
                || split.contains("开课")
                || split.contains("存档"),
            "split bodies should follow scene lexicon"
        );
        assert!(
            planner_greenfield_stack_blurb().contains("假设")
                || planner_greenfield_stack_blurb().contains("人话"),
            "planner greenfield: assumptions / human copy"
        );
    }

    #[test]
    fn chat_guidance_forbids_placeholder_images() {
        let g = chat_plan_writing_guidance();
        assert!(
            g.contains("占位图") || g.contains("placehold"),
            "must ban placeholder images"
        );
        assert!(
            g.contains("图库") || g.contains("生成") || g.contains("Unsplash") || g.contains("Pexels"),
            "must allow stock or generated art"
        );
    }

    #[test]
    fn ui_delivery_recipes_covers_combos() {
        let g = ui_delivery_recipes_guidance();
        assert!(g.contains("R-overseas"));
        assert!(g.contains("R-shanshui") || g.contains("R-cn-brand"));
        assert!(g.contains("R-tool") || g.contains("R-admin"));
        assert!(g.contains("R-ios") || g.contains("ios-hig"));
        assert!(g.contains("R-material") || g.contains("material"));
        assert!(g.contains("R-fluent") || g.contains("R-wechat") || g.contains("R-ant"));
        assert!(g.contains("western-saas") || g.contains("色系"));
        assert!(g.contains("图片") || g.contains("Hero"));
        assert!(g.contains("后端") || g.contains("交付深度"));
        assert!(
            g.contains("site-floor") || g.contains("demos/r-overseas"),
            "recipes must link runnable site-floor"
        );
    }

    #[test]
    fn ui_color_covers_platform_kits() {
        let g = ui_color_systems_guidance();
        assert!(g.contains("ios-hig"), "ios kit");
        assert!(g.contains("material"), "material kit");
        assert!(g.contains("fluent") || g.contains("Fluent"));
        assert!(g.contains("ant-design") || g.contains("wechat"));
    }

    #[test]
    fn ui_layout_covers_site_types() {
        let g = ui_layout_systems_guidance();
        assert!(g.contains("marketing"));
        assert!(g.contains("portfolio") || g.contains("作品集"));
        assert!(g.contains("dashboard") || g.contains("后台"));
        assert!(g.contains("content") || g.contains("文档"));
        assert!(g.contains("story") || g.contains("叙事"));
        assert!(g.contains("Hero") || g.contains("首屏"));
        assert!(
            g.contains("受控变化") || g.contains("版式变体") || g.contains("防死板"),
            "controlled variation for non-rigid layouts"
        );
    }

    #[test]
    fn ui_color_systems_covers_kits() {
        let g = ui_color_systems_guidance();
        assert!(
            g.contains("western-saas"),
            "got head: {}",
            &g[..g.len().min(120)]
        );
        assert!(g.contains("shanshui") || g.contains("山水"));
        assert!(g.contains("jp-wa") || g.contains("和风"));
        assert!(g.contains("--color-primary") || g.contains("color-primary"));
        assert!(g.contains("cta-band") || g.contains("cta_band") || g.contains("CTA"));
    }

    #[test]
    fn ui_typography_covers_roles_and_shanshui() {
        let g = ui_typography_systems_guidance();
        assert!(g.contains("display") || g.contains("--font-display"));
        assert!(g.contains("body") || g.contains("--font-body"));
        assert!(g.contains("文楷") || g.contains("LXGW") || g.contains("楷"));
        assert!(g.contains("Inter") || g.contains("western-saas"));
        assert!(g.contains("shanshui") || g.contains("山水"));
    }

    #[test]
    fn ui_motion_covers_tiers_and_whitelist() {
        let g = ui_motion_effects_guidance();
        assert!(g.contains("light") || g.contains("动效档"));
        assert!(g.contains("GSAP") || g.contains("anime"));
        assert!(g.contains("prefers-reduced-motion") || g.contains("reduced-motion"));
        assert!(g.contains("tsparticles") || g.contains("Three") || g.contains("Lottie"));
        assert!(g.contains("western-saas") || g.contains("shanshui"));
    }

    #[test]
    fn backend_architecture_covers_depths_and_langs() {
        let g = backend_architecture_guidance();
        assert!(g.contains("演示") || g.contains("交付深度"), "depth");
        assert!(g.contains("Node") || g.contains("Go"));
        assert!(g.contains("MVC") || g.contains("DDD"));
        assert!(g.contains("Rust") || g.contains("Java") || g.contains("PHP"));
        assert!(!g.contains("一律 DDD"), "must not force DDD for demos");
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
