//! Human-facing task title / summary / done_when / dep cell parsing.
//!
//! [INPUT]: task title + plan body (or worker-wrapped prompt)
//! [OUTPUT]: display strings for split desk (甲/乙 default layer)
//! [POS]: domain/plan/cco_split — pure; used by convert + heuristic
//! [PROTOCOL]: 变更时更新此头部

use std::collections::HashMap;

/// Strip plan checkbox / status marks from titles for display.
pub fn display_title(title: &str) -> String {
    let mut s = title.trim().to_string();
    // Trailing status glyphs common in landing plans.
    loop {
        let t = s.trim_end();
        let stripped = t
            .trim_end_matches(['☐', '✅', '☑', '□', '■', '✗', '✘', '×'])
            .trim_end();
        if stripped.len() == t.len() {
            s = stripped.to_string();
            break;
        }
        s = stripped.to_string();
    }
    s.trim().to_string()
}

/// True if a line is worker scaffold / contract noise (must not be one-liner).
pub fn is_worker_noise_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    t.starts_with("你是执行")
        || t.contains("的 worker")
        || t.contains("的worker")
        || lower.contains("cco_done")
        || t.starts_with("项目根目录")
        || t.starts_with("依据下列说明")
        || t.starts_with("全部完成后在最后一行")
        || t.starts_with("完成后输出一行")
        || t.starts_with("--- 本阶段说明")
        || t.starts_with("## 任务说明")
        || t.starts_with("## 依赖原因")
        || t.starts_with("等待前置步骤产物")
}

/// Drop planner worker wrapper so body/table remain.
pub fn strip_worker_scaffold(prompt: &str) -> String {
    let s = prompt.trim();
    if s.is_empty() {
        return String::new();
    }
    // Prefer content under ## 任务说明
    if let Some(idx) = s.find("## 任务说明") {
        let rest = s[idx + "## 任务说明".len()..].trim_start();
        let rest = rest
            .trim_start_matches(|c: char| c == '\n' || c == '\r')
            .to_string();
        let cut = rest
            .find("全部完成后在最后一行")
            .or_else(|| rest.find("完成后输出一行"))
            .unwrap_or(rest.len());
        let body = rest[..cut].trim().to_string();
        if body.len() > 8 {
            return body;
        }
    }
    // Drop leading worker identity block until first ## or table
    let mut out = String::new();
    let mut skipping = true;
    for line in s.lines() {
        let t = line.trim();
        if skipping {
            if is_worker_noise_line(t) || t.is_empty() {
                continue;
            }
            if t.starts_with("## ") || t.starts_with('|') || t.starts_with("---") {
                skipping = false;
            } else if t.starts_with("按**本阶段") || t.starts_with("按**本阶段") {
                continue;
            } else {
                skipping = false;
            }
        }
        if t.starts_with("全部完成后在最后一行") || t.starts_with("完成后输出一行")
        {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        s.to_string()
    } else {
        out
    }
}

/// Parse markdown table cell for a labeled row (`| **完成定义** | … |`).
pub fn parse_table_field(body: &str, labels: &[&str]) -> Option<String> {
    let body = strip_worker_scaffold(body);
    for line in body.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = t
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .collect();
        if cells.len() < 2 {
            continue;
        }
        let key = cells[0]
            .trim_matches('*')
            .trim()
            .trim_matches('*')
            .trim()
            .to_string();
        let key_norm = key.replace(' ', "");
        for lab in labels {
            let lab_norm = lab.replace(' ', "");
            if key == *lab || key_norm == lab_norm || key.contains(lab) {
                let val = cells[1..].join(" · ").trim().to_string();
                if !val.is_empty() && val != "—" && val != "-" {
                    return Some(val);
                }
            }
        }
    }
    None
}

pub fn parse_done_when(body: &str) -> Option<String> {
    parse_table_field(
        body,
        &[
            "完成定义",
            "完成判据",
            "成功标准",
            "验收",
            "Acceptance",
            "acceptance",
            "Done when",
            "done when",
        ],
    )
    .map(|s| truncate_chars(&s, 160))
}

pub fn parse_dep_cell(body: &str) -> Option<String> {
    parse_table_field(body, &["依赖", "depends", "Depends", "前置", "等待"])
}

pub fn dep_cell_is_none(cell: &str) -> bool {
    let t = cell.trim();
    if t.is_empty() || t == "—" || t == "-" || t == "无" || t.eq_ignore_ascii_case("none") {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    lower.starts_with("无（")
        || lower.starts_with("无(")
        || lower.contains("无依赖")
        || lower.contains("无强依赖")
        || lower.starts_with("可与")
        || lower.contains("可并行")
}

/// Work-package ids like P0-1 / A1 / U1-1 / PR-A inside free text.
pub fn extract_work_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // ASCII letter(s) + optional digits + - + digits… e.g. P0-1, U1-1, PR-A
        if bytes[i].is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let mut parts = 0;
            while i < bytes.len() && bytes[i] == b'-' {
                i += 1;
                let p0 = i;
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                if i > p0 {
                    parts += 1;
                } else {
                    break;
                }
            }
            if parts >= 1 {
                if let Ok(s) = std::str::from_utf8(&bytes[start..i]) {
                    // Avoid matching plain words: need digit or multi-part
                    if s.chars().any(|c| c.is_ascii_digit()) || s.contains('-') {
                        let id = s.to_string();
                        if !out.iter().any(|x| x == &id) {
                            out.push(id);
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Map work-package id found in title (P0-1) → task id (t1).
pub fn work_id_from_title(title: &str) -> Option<String> {
    let t = display_title(title);
    // Prefer ####-style leading id
    let ids = extract_work_ids(&t);
    ids.into_iter().next()
}

/// Build depends_on from parsed 依赖 cells using title work-ids.
/// Returns (deps_per_section_index, any_explicit_dep_info).
pub fn resolve_deps_from_sections(sections: &[(String, String)]) -> (Vec<Vec<String>>, bool) {
    let mut work_to_task: HashMap<String, String> = HashMap::new();
    for (i, (title, _)) in sections.iter().enumerate() {
        let tid = format!("t{}", i + 1);
        if let Some(wid) = work_id_from_title(title) {
            work_to_task.insert(wid.to_ascii_uppercase(), tid.clone());
            work_to_task.insert(wid, tid);
        }
    }

    let mut any_info = false;
    let mut deps: Vec<Vec<String>> = Vec::with_capacity(sections.len());
    for (i, (_title, body)) in sections.iter().enumerate() {
        let tid = format!("t{}", i + 1);
        let mut d = Vec::new();
        if let Some(cell) = parse_dep_cell(body) {
            any_info = true;
            if !dep_cell_is_none(&cell) {
                for wid in extract_work_ids(&cell) {
                    let key_up = wid.to_ascii_uppercase();
                    if let Some(dep_tid) =
                        work_to_task.get(&key_up).or_else(|| work_to_task.get(&wid))
                    {
                        if dep_tid != &tid && !d.contains(dep_tid) {
                            d.push(dep_tid.clone());
                        }
                    }
                }
                // Also match by title substring (P0-1 · …)
                for (j, (ot, _)) in sections.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    let other_id = format!("t{}", j + 1);
                    if d.contains(&other_id) {
                        continue;
                    }
                    if let Some(ow) = work_id_from_title(ot) {
                        if cell.contains(&ow) {
                            d.push(other_id);
                        }
                    }
                }
            }
        }
        deps.push(d);
    }
    (deps, any_info)
}

/// Human one-liner for split cards (never worker scaffold).
pub fn human_summary(title: &str, body: &str, acceptance: Option<&str>) -> String {
    if let Some(a) = acceptance.map(str::trim).filter(|s| !s.is_empty()) {
        // Prefer a short intent from title if acceptance is long criteria
        let t = display_title(title);
        if !t.is_empty() && t.chars().count() <= 48 {
            return truncate_chars(&t, 72);
        }
        return truncate_chars(a, 72);
    }
    if let Some(done) = parse_done_when(body) {
        let t = display_title(title);
        if !t.is_empty() && t.chars().count() <= 48 {
            return truncate_chars(&t, 72);
        }
        return truncate_chars(&done, 72);
    }
    // Intent from steps row first sentence
    if let Some(steps) = parse_table_field(body, &["步骤", "改法", "Steps"]) {
        let first = steps
            .split(['。', '；', ';', '\n'])
            .map(str::trim)
            .find(|s| s.chars().count() > 4)
            .unwrap_or(&steps);
        let cleaned = first
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '、' || c == ' ')
            .trim();
        if cleaned.chars().count() > 4 && !is_worker_noise_line(cleaned) {
            return truncate_chars(cleaned, 72);
        }
    }
    let t = display_title(title);
    if !t.is_empty() {
        return truncate_chars(&t, 72);
    }
    let plain = strip_worker_scaffold(body);
    for line in plain.lines() {
        let l = line.trim().trim_start_matches(['#', '-', '*', '|']).trim();
        if l.len() > 4 && !is_worker_noise_line(l) && !l.starts_with("---") {
            return truncate_chars(l, 72);
        }
    }
    "查看步骤说明".into()
}

fn truncate_chars(s: &str, max: usize) -> String {
    let s = s.trim();
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let take: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{take}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_checkbox_from_title() {
        assert_eq!(
            display_title("P0-1 · 结果台消费 live 费用与用时 ☐"),
            "P0-1 · 结果台消费 live 费用与用时"
        );
    }

    #[test]
    fn summary_not_worker() {
        let body = r#"你是执行任务 `t1`（P0-1）的 worker。
项目根目录即当前工作目录。

## 任务说明
| 项 | 内容 |
|----|------|
| **完成定义** | 终态结果台可见人话费用或「未汇总」 |
| **依赖** | 无（live 字段已有） |
"#;
        let s = human_summary("P0-1 · 结果台消费 live 费用与用时 ☐", body, None);
        assert!(!s.contains("worker"), "{s}");
        assert!(!s.contains("你是执行"), "{s}");
    }

    #[test]
    fn parse_done_and_none_dep() {
        let body = r#"
| 项 | 内容 |
|----|------|
| **完成定义** | 打开 report 第一行无人话以外的 run_id |
| **依赖** | 无 |
"#;
        assert!(parse_done_when(body).unwrap().contains("report"));
        assert!(dep_cell_is_none(&parse_dep_cell(body).unwrap()));
    }

    #[test]
    fn resolve_deps_p0_chain() {
        let sections = vec![
            (
                "P0-1 · a ☐".into(),
                "| **依赖** | 无 |\n| **完成定义** | done1 |".into(),
            ),
            (
                "P0-2 · b ☐".into(),
                "| **依赖** | 无 |\n| **完成定义** | done2 |".into(),
            ),
            ("P0-4 · c ☐".into(), "| **依赖** | P0-1 · P0-3 |\n".into()),
            (
                "P0-3 · d ☐".into(),
                "| **依赖** | P0-2 标题可同 PR |\n".into(),
            ),
        ];
        let (deps, any) = resolve_deps_from_sections(&sections);
        assert!(any);
        assert!(deps[0].is_empty());
        assert!(deps[1].is_empty());
        // P0-4 depends on P0-1 (t1) and P0-3 (t4)
        assert!(deps[2].contains(&"t1".to_string()));
        assert!(deps[2].contains(&"t4".to_string()) || deps[2].len() >= 1);
        assert!(deps[3].contains(&"t2".to_string()));
    }
}
