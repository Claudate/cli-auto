//! Plan title extract / sanitize (G0).

/// Max chars for list / rail titles (G0). Longer H1s get ellipsis.
pub const PLAN_TITLE_MAX_CHARS: usize = 80;

/// Sanitize a raw H1 body into a short list title.
/// Cuts at embedded `##` (single-line "wall" plans) and clamps length.
pub fn sanitize_plan_title(raw: &str) -> String {
    let mut s = raw.trim();
    // Single-line dumps often jam "# Title## 目标…" — stop before next heading.
    if let Some(idx) = s.find("##") {
        s = s[..idx].trim_end();
    }
    // Also stop at an accidental second "# " mid-string (rare).
    if let Some(idx) = s.find("\n# ") {
        s = s[..idx].trim_end();
    }
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let count = s.chars().count();
    if count <= PLAN_TITLE_MAX_CHARS {
        return s.to_string();
    }
    format!(
        "{}…",
        s.chars().take(PLAN_TITLE_MAX_CHARS).collect::<String>()
    )
}

/// Extract short plan title from markdown (H1). Safe for no-newline walls (G0).
pub fn extract_title_from_md(md: &str) -> Option<String> {
    for line in md.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = sanitize_plan_title(rest);
            if !title.is_empty() {
                return Some(title);
            }
        } else if let Some(rest) = t.strip_prefix('#') {
            // "#Title" without space
            let rest = rest.trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                let title = sanitize_plan_title(rest);
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }
    // Whole file may be one line with no \n — lines() still yields it once.
    None
}
