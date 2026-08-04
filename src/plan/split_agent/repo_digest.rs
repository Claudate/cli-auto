//! Bounded read-only repo digest for ModelSplitAgent (host-side, not a worker).
//!
//! [INPUT]: project root · plan markdown
//! [OUTPUT]: short text block for split user_prompt (empty on failure)
//! [POS]: plan/split_agent
//! [PROTOCOL]: 只读 · 有上限 · 失败静默空串；禁止当业务节点 / 不开跑
//!
//! Purpose: improve scope_paths quality without full-repo explore.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_HINTS: usize = 12;
const MAX_TOP: usize = 16;
const MAX_CHILDREN: usize = 8;
const MAX_CHARS: usize = 1800;

/// Path-ish tokens mentioned in the plan (repo-relative style).
pub fn extract_path_hints(plan_md: &str) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for raw in plan_md.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '`' | '"'
                    | '\''
                    | ','
                    | ';'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '|'
                    | '，'
                    | '。'
                    | '、'
                    | '；'
                    | '：'
                    | '（'
                    | '）'
            )
    }) {
        let t = raw
            .trim()
            .trim_matches(|c| matches!(c, '*' | '`' | '"' | '\''));
        if t.len() < 3 || t.len() > 120 {
            continue;
        }
        if looks_like_repo_path(t) {
            let cleaned = t.trim_start_matches("./").to_string();
            if !cleaned.is_empty() {
                out.insert(cleaned);
            }
        }
        if out.len() >= MAX_HINTS * 2 {
            break;
        }
    }
    out.into_iter().take(MAX_HINTS).collect()
}

fn looks_like_repo_path(s: &str) -> bool {
    if s.contains("://") || s.starts_with('/') || s.starts_with('~') {
        return false;
    }
    if s.contains("..") {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    // Common monorepo / cco layout prefixes or extension files.
    let prefix_ok = [
        "src/",
        "src-",
        "web/",
        "docs/",
        "tests/",
        "scripts/",
        "examples/",
        "crates/",
        "app/",
        "apps/",
        "packages/",
        "frontend/",
        "backend/",
        "lib/",
        "cmd/",
        "internal/",
        ".cco",
        "cargo.toml",
        "package.json",
        "readme",
        "claude.md",
    ]
    .iter()
    .any(|p| lower.starts_with(p) || lower == *p);
    if prefix_ok {
        return true;
    }
    // path with slash + extension or trailing /**
    if s.contains('/')
        && (s.contains('.')
            || s.ends_with("/**")
            || s.ends_with('/')
            || s.chars().any(|c| c == '_'))
    {
        return !s.starts_with("http");
    }
    false
}

/// Build a short digest. Never errors to caller — empty string on any problem.
pub fn build_repo_digest(project: &Path, plan_md: &str) -> String {
    if !project.is_dir() {
        return String::new();
    }
    let mut lines: Vec<String> = Vec::new();
    lines.push("仓库浅览（只读 · 供 scope_paths 参考；路径不存在可忽略）：".into());

    // Top-level names (skip dot/target/node_modules).
    if let Ok(rd) = std::fs::read_dir(project) {
        let mut tops: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "dist"
                    || name == ".git"
                {
                    return None;
                }
                let mark = if e.path().is_dir() { "/" } else { "" };
                Some(format!("{name}{mark}"))
            })
            .collect();
        tops.sort();
        tops.truncate(MAX_TOP);
        if !tops.is_empty() {
            lines.push(format!("顶层：{}", tops.join(" · ")));
        }
    }

    let hints = extract_path_hints(plan_md);
    if hints.is_empty() {
        lines.push("计划未点名具体路径；请按顶层与常见布局推断 scope。".into());
    } else {
        lines.push(format!("计划点名路径：{}", hints.join(" · ")));
        for h in hints.iter().take(8) {
            let rel = h.trim_end_matches('/').trim_end_matches("/**");
            let p = project.join(rel);
            if p.is_file() {
                lines.push(format!("  · {rel} 存在（文件）"));
            } else if p.is_dir() {
                let kids = list_children_brief(&p, MAX_CHILDREN);
                if kids.is_empty() {
                    lines.push(format!("  · {rel}/ 存在（空或不可读）"));
                } else {
                    lines.push(format!("  · {rel}/ → {}", kids.join(", ")));
                }
            } else {
                // Try parent dir if glob-like
                if let Some(parent) = Path::new(rel).parent() {
                    if !parent.as_os_str().is_empty() {
                        let pp = project.join(parent);
                        if pp.is_dir() {
                            let kids = list_children_brief(&pp, 4);
                            lines.push(format!(
                                "  · {rel} 未找到；父目录 {}/ 有：{}",
                                parent.display(),
                                if kids.is_empty() {
                                    "（空）".into()
                                } else {
                                    kids.join(", ")
                                }
                            ));
                            continue;
                        }
                    }
                }
                lines.push(format!("  · {rel} 未在仓库找到（拆分时勿硬编不存在路径）"));
            }
        }
    }

    let mut s = lines.join("\n");
    if s.chars().count() > MAX_CHARS {
        s = s.chars().take(MAX_CHARS).collect::<String>() + "…";
    }
    s
}

fn list_children_brief(dir: &Path, max: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return names;
    };
    for e in rd.filter_map(|e| e.ok()).take(max + 4) {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if e.path().is_dir() {
            names.push(format!("{name}/"));
        } else {
            names.push(name);
        }
        if names.len() >= max {
            break;
        }
    }
    names.sort();
    names
}

/// Resolve project path for digest (canonicalize best-effort).
pub fn project_for_digest(req_project: &Path) -> PathBuf {
    req_project
        .canonicalize()
        .unwrap_or_else(|_| req_project.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn extracts_common_paths() {
        let md = "# x\n改 `src/app/split.rs` 与 web/js/features/result/** 以及 docs/foo.md\n";
        let h = extract_path_hints(md);
        assert!(h.iter().any(|p| p.contains("src/app")), "{h:?}");
        assert!(
            h.iter()
                .any(|p| p.contains("web/js") || p.contains("result")),
            "{h:?}"
        );
    }

    #[test]
    fn ignores_urls_and_abs() {
        let md = "see https://example.com/src/foo and /etc/passwd and ~/secret";
        let h = extract_path_hints(md);
        assert!(h.is_empty(), "{h:?}");
    }

    #[test]
    fn digest_lists_top_and_hint() {
        let dir = std::env::temp_dir().join(format!(
            "cco-digest-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(dir.join("src/app")).unwrap();
        fs::write(dir.join("src/app/mod.rs"), "//").unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"t\"\n").unwrap();
        let d = build_repo_digest(&dir, "请改 src/app/mod.rs 与不存在的 web/nope.js");
        assert!(d.contains("顶层"), "{d}");
        assert!(d.contains("src/app"), "{d}");
        let _ = fs::remove_dir_all(&dir);
    }
}
