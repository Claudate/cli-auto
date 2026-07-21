//! Plan markdown normalize / local structure fill (G0 / G0b local).

use super::title::extract_title_from_md;

/// Normalize plan markdown before disk write (G0).
/// - Unify newlines
/// - If essentially one line, insert breaks before `##` / `###` headings
pub fn normalize_plan_markdown(md: &str) -> String {
    let mut s = md.replace("\r\n", "\n").replace('\r', "\n");
    let nl = s.matches('\n').count();
    if nl <= 1 && s.chars().count() > 60 {
        // Recover jammed single-line structure for Mode B + human read.
        s = s.replace("### ", "\n\n### ");
        s = s.replace("## ", "\n\n## ");
        s = s.trim().to_string();
        // Ensure H1 is followed by blank line when next is ##
        if let Some(rest) = s.strip_prefix("# ") {
            if let Some(pos) = rest.find("\n\n##") {
                let title = &rest[..pos];
                let body = &rest[pos..];
                s = format!("# {}\n{}", title.trim_end(), body);
            } else if !rest.contains('\n') {
                // still one line after ## inject failed (no ##) — keep as is
            }
        }
    }
    // Guarantee trailing newline
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// G0b local: ensure draft has short H1 + core sections (no CLI).
/// Idempotent when already structured; fills missing headings only.
pub fn structure_plan_markdown(md: &str) -> String {
    let mut s = normalize_plan_markdown(md);
    let lower = s.to_lowercase();
    let has_h1 = s.lines().any(|l| {
        let t = l.trim();
        t.starts_with("# ") || (t.starts_with('#') && !t.starts_with("##"))
    });
    if !has_h1 {
        let title = extract_title_from_md(&s).unwrap_or_else(|| "聊天生成计划".into());
        s = format!("# {title}\n\n{s}");
    }
    // Re-extract short title and rewrite first H1 if wall-like
    if let Some(title) = extract_title_from_md(&s) {
        if let Some(rest_start) = s.find('\n') {
            let rest = &s[rest_start..];
            s = format!("# {title}{rest}");
        } else {
            s = format!("# {title}\n");
        }
    }
    let mut missing = Vec::new();
    if !lower.contains("## 目标") && !lower.contains("## goal") {
        missing.push("## 目标\n（请补充 1～3 句目标）\n");
    }
    if !lower.contains("## 范围") && !lower.contains("## scope") {
        missing.push("## 范围\n- 做：…\n- 不做：…\n");
    }
    if !lower.contains("## 任务") && !lower.contains("## tasks") {
        missing.push("## 任务大纲\n### T1 · （可执行标题）\n- 说明：…\n- 验收：…\n");
    }
    if !lower.contains("## 验收") && !lower.contains("## acceptance") {
        missing.push("## 验收（整计划）\n- [ ] …\n");
    }
    if !missing.is_empty() {
        s = s.trim_end().to_string();
        s.push_str("\n\n---\n\n");
        s.push_str(&missing.join("\n"));
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}
