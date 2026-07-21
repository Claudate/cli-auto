//! Optional-task titles and meta-heading heuristics (pure).
//!
//! [INPUT]: title 字符串
//! [OUTPUT]: normalize_optional_title · title_looks_optional · title_is_meta_heading
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部

/// Ensure optional tasks have a clear title marker; required tasks stay as-is.
pub fn normalize_optional_title(title: &str, optional: bool) -> String {
    let t = title.trim();
    if !optional {
        return t.to_string();
    }
    let lower = t.to_ascii_lowercase();
    if t.contains("可选") || lower.contains("optional") {
        t.to_string()
    } else if t.is_empty() {
        "（可选）".into()
    } else {
        format!("{t}（可选）")
    }
}

/// Detect optional intent from a free-form title (planner / heading split).
pub fn title_looks_optional(title: &str) -> bool {
    let t = title.trim();
    let lower = t.to_ascii_lowercase();
    t.contains("可选") || lower.contains("optional") || lower.contains("(opt)")
}

/// True when a heading/title is document chrome — not a work package.
/// Used by Mode B planner (LLM reject + heuristic skip) so users who only
/// supply Markdown specs don't see Board / P0 / 修订历史 as runnable tasks.
pub fn title_is_meta_heading(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return true;
    }
    // Markdown table header / pipe row (e.g. "id | provider | role | …")
    let pipes = t.chars().filter(|c| *c == '|').count();
    if pipes >= 2 {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    let compact: String = lower
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '·' && *c != '•' && *c != '—' && *c != '-')
        .collect();

    // Bare structural labels (handoff template, TOC, plan chrome)
    const EXACT: &[&str] = &[
        "board",
        "timeline",
        "fragments",
        "graph",
        "tasks",
        "task",
        "toc",
        "目录",
        "overview",
        "summary",
        "notes",
        "readme",
        "前言",
        "全文",
        "附录",
        "附录a",
        "附录b",
        "appendix",
        "protocol",
        "一句话",
        "0.一句话",
    ];
    if EXACT.iter().any(|k| compact == *k || lower == *k) {
        return true;
    }

    // Phrase / prefix patterns common in product-spec Markdown (not work orders)
    const NEEDLES: &[&str] = &[
        "修订历史",
        "revision history",
        "非目标",
        "non-goal",
        "nongoal",
        "成功标准",
        "决策树",
        "决策默认",
        "开放确认",
        "关联真源",
        "代码锚点",
        "现状锚点",
        "现状分析",
        "产品结论",
        "协作契约",
        "阶段切分",
        "架构落点",
        "风险与决策",
        "附录",
        "appendix",
        // Bare handoff/board as substring would false-positive real work titles
        // like「实现 handoff 归并」— those are EXACT-only above.
        "instructions for next",
        "open risks",
        "protocol",
        "geb",
        "勾选",
        "p0 —",
        "p1 —",
        "p2 —",
        "p0-",
        "p1-",
        "p2-",
        "§",
        // Phase banners from product plans (title without leading P0)
        "协议与示例",
        "host 硬保障",
        "硬保障（代码）",
        "检验员与分配",
        "分配体验",
        "文档 / 示例",
        "文档/示例",
    ];
    if NEEDLES
        .iter()
        .any(|n| lower.contains(n) || compact.contains(&n.replace(' ', "")))
    {
        return true;
    }
    // "…（按需）" / "…(按需)" stage banners without a work verb
    if (lower.contains("按需") || lower.contains("可选增强"))
        && !["实现", "落地", "修复", "新增", "编写", "接入", "改造", "测试", "验收"]
            .iter()
            .any(|c| lower.contains(c))
    {
        return true;
    }

    // Leading "N. " / "N " section numbers that are pure catalog titles
    // e.g. "6. 阶段切分与勾选" already hit 阶段切分; also "12. 修订历史"
    if looks_like_numbered_catalog_title(&lower) {
        return true;
    }

    // Stage-only labels: "P0 协议与示例" without a verb-ish work cue
    if is_stage_catalog_title(&lower) {
        return true;
    }

    false
}

fn looks_like_numbered_catalog_title(lower: &str) -> bool {
    let t = lower.trim();
    // "0. 一句话" / "8. 非目标" / "10. 成功标准"
    let rest = if let Some(r) = t.strip_prefix(|c: char| c.is_ascii_digit()) {
        let r = r.trim_start_matches(|c: char| c.is_ascii_digit());
        r.strip_prefix('.').or_else(|| r.strip_prefix('、')).unwrap_or(r).trim()
    } else {
        return false;
    };
    const CATALOG: &[&str] = &[
        "一句话",
        "产品结论",
        "现状",
        "协作契约",
        "端到端",
        "计划与配置",
        "阶段",
        "架构",
        "非目标",
        "风险",
        "成功标准",
        "决策",
        "修订历史",
        "附录",
        "拍板",
        "分配策略",
        "档位",
        "勾选",
        "示例",
        "配置",
        "主路径",
        "契约",
        "落点",
        "总览",
        "决策树",
        "决策默认",
    ];
    CATALOG.iter().any(|c| rest.starts_with(c) || rest.contains(c))
}

fn is_stage_catalog_title(lower: &str) -> bool {
    let t = lower.trim();
    let stage = t.starts_with("p0")
        || t.starts_with("p1")
        || t.starts_with("p2")
        || t.starts_with("m0")
        || t.starts_with("m1")
        || t.starts_with("m2")
        || t.starts_with("m3")
        || t.starts_with("m4")
        || t.starts_with("m5")
        || t.starts_with("d0")
        || t.starts_with("d1")
        || t.starts_with("d2")
        || t.starts_with("d3")
        || t.starts_with("d4")
        || t.starts_with("d5");
    if !stage {
        return false;
    }
    // Allow real work titles like "P0 实现 handoff 归并" (has action-ish length + 实现)
    let work_cues = ["实现", "落地", "修复", "新增", "编写", "接入", "改造", "测试", "验收"];
    if work_cues.iter().any(|c| t.contains(c)) {
        return false;
    }
    // Short stage banners: "p0 — 协议与示例（文档 / 示例为主）"
    true
}

