//! serial-prompts/v0 Markdown adapter.
//!
//! [INPUT]: 多段 Markdown 提示词
//! [OUTPUT]: 串行依赖 PlanIR
//! [POS]: 半结构化计划适配；有 golden fixture
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/adapters/CLAUDE.md

use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::config::Config;
use crate::plan::adapters::raw_single::{self, default_provider_opts};
use crate::plan::{OnFailure, PlanIR, TaskIR};

pub fn parse(path: &Path, text: &str, config: &Config) -> Result<PlanIR> {
    let provider = config.default.default_provider.clone();
    let mode = config.default.default_mode.clone();
    let opts = default_provider_opts(config, &provider);

    // Match ## or ### task headings (common in multi-window prompt docs).
    let heading_re =
        Regex::new(r"(?m)^#{2,3}\s+([A-Za-z0-9_.-]+)(?:\s*[·•\-—|]\s*(.+))?$").unwrap();
    let fence_re = Regex::new(r"(?s)```[^\n]*\n(.*?)```").unwrap();

    // Collect (id, title, body_start)
    let mut heads: Vec<(usize, String, String)> = Vec::new();
    for cap in heading_re.captures_iter(text) {
        let whole = cap.get(0).unwrap();
        let id = cap[1].to_string();
        let title = cap
            .get(2)
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| id.clone());
        // skip document chrome (Board / Timeline / P0 勾选 / 表头…) — not work packages
        if crate::plan::title_is_meta_heading(&id)
            || crate::plan::title_is_meta_heading(&title)
        {
            continue;
        }
        heads.push((whole.start(), id, title));
    }

    let deps_map = parse_deps_table(text);

    let mut tasks = Vec::new();
    for (i, (start, id, title)) in heads.iter().enumerate() {
        let end = heads
            .get(i + 1)
            .map(|(s, _, _)| *s)
            .unwrap_or(text.len());
        let section = &text[*start..end];
        let prompt = fence_re
            .captures(section)
            .map(|c| c[1].trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| section.to_string());

        let depends_on = deps_map.get(id).cloned().unwrap_or_default();

        let optional = crate::plan::title_looks_optional(&title);
        let title = crate::plan::normalize_optional_title(&title, optional);
        tasks.push(TaskIR {
            id: id.clone(),
            title,
            depends_on,
            group: None,
            provider: provider.clone(),
            mode: mode.clone(),
            prompt,
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: Some(config.default.worktree),
            provider_opts: opts.clone(),
            optional,
            include: !optional,
            role: None,
            scope: None,
            outputs: vec![],
        tags: vec![],
        });
    }

    if tasks.is_empty() {
        return raw_single::parse(path, text, config);
    }

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("serial")
        .to_string();

    Ok(PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "serial-prompts/v0".into(),
        source_path: path.to_path_buf(),
        max_parallel: config.default.max_parallel,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: provider,
        default_mode: mode,
        worktree: config.default.worktree,
        require_inspect: false,
        tasks,
    })
}

/// Parse markdown tables that have id + depends_on-like columns.
fn parse_deps_table(text: &str) -> std::collections::HashMap<String, Vec<String>> {
    let mut map = std::collections::HashMap::new();
    let mut depend_col: Option<usize> = None;

    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if cols.len() < 2 {
            continue;
        }
        // separator
        if cols
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        // header row
        let header_like = cols.iter().any(|c| {
            let l = c.to_ascii_lowercase();
            l == "id" || l.contains("title") || l.contains("依赖") || l.contains("depend")
        });
        if header_like {
            let headers: Vec<String> = cols.iter().map(|s| s.to_ascii_lowercase()).collect();
            depend_col = headers.iter().position(|h| {
                h.contains("depend") || h.contains("依赖") || h == "deps" || h == "requires"
            });
            continue;
        }

        let id = cols[0].to_string();
        if id.len() > 32
            || !id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            continue;
        }

        let mut deps = Vec::new();
        // Prefer explicit depends_on column when header known
        if let Some(di) = depend_col {
            if let Some(cell) = cols.get(di) {
                deps.extend(split_dep_cell(cell));
            }
        } else {
            // fallback: scan cells that look like id lists
            for c in cols.iter().skip(1) {
                let l = c.to_ascii_lowercase();
                if l == "print" || l == "bg" || l == "auto" {
                    continue;
                }
                if l.starts_with('g') && l.len() <= 3 {
                    continue; // group ids like G1
                }
                if c.contains(',') || (c.starts_with('t') && c.len() <= 8) {
                    deps.extend(split_dep_cell(c));
                }
            }
        }
        deps.retain(|d| d != &id && d != "print" && d != "bg" && d != "auto");
        map.insert(id, deps);
    }
    map
}

fn split_dep_cell(cell: &str) -> Vec<String> {
    cell.split(|ch: char| ch == ',' || ch == ' ' || ch == '、' || ch == ';')
        .map(|s| s.trim())
        .filter(|p| {
            !p.is_empty()
                && *p != "-"
                && *p != "—"
                && p.len() <= 32
                && p.chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        })
        .map(|s| s.to_string())
        .collect()
}
