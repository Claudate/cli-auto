//! ```plan fence extract — CJK-safe · nested line-start depth (F0/F1).

/// Byte length of a markdown fence language tag at the start of `after`.
/// Tag is ASCII `[A-Za-z0-9_+-]*` only, so the returned index is always a char boundary.
fn fence_lang_tag_len(after: &str) -> usize {
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '+' || *c == '-')
        .map(|c| c.len_utf8())
        .sum()
}

/// True when `idx` is at the start of `s` or immediately after `\n` / `\r`.
/// Markdown fences are line-oriented; mid-line `` ` `` sequences are ignored.
fn is_line_start_fence(s: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    matches!(s.as_bytes().get(idx.saturating_sub(1)), Some(b'\n' | b'\r'))
}

/// Find the next line-start ``` fence at or after `from` (byte index into `s`).
/// Returns `None` if none remain. ``` is ASCII → returned index is a char boundary.
fn find_line_fence(s: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if from >= bytes.len() {
        return None;
    }
    let mut i = from;
    // If `from` is mid-line, jump to the next line first.
    if i > 0 && !matches!(bytes.get(i.saturating_sub(1)), Some(b'\n' | b'\r')) {
        if let Some(rel) = s[i..].find(|c| c == '\n' || c == '\r') {
            i += rel + 1;
            // handle \r\n
            if i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n' {
                i += 1;
            }
        } else {
            return None;
        }
    }
    while i < bytes.len() {
        if s[i..].starts_with("```") && is_line_start_fence(s, i) {
            return Some(i);
        }
        // next line
        if let Some(rel) = s[i..].find(|c| c == '\n' || c == '\r') {
            i += rel + 1;
            if i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n' {
                i += 1;
            }
        } else {
            break;
        }
    }
    None
}

/// Close a fence body starting at `body` (content after opener tag + newline trim).
/// Supports **nested** fenced blocks (```text / ``` / ```rust inside ```plan).
///
/// Returns `(body_end_byte_in_body, absolute_scan_continue_from_body)` when closed.
fn close_fence_body(body: &str) -> Option<(usize, usize)> {
    let mut depth: i32 = 1;
    let mut pos = 0usize;
    while let Some(j) = find_line_fence(body, pos) {
        let after = &body[j + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        // Opening if tag non-empty (```text / ```rust / ```plan …);
        // closing if bare ``` (optional trailing spaces/newline only after tag).
        if !tag.is_empty() {
            depth += 1;
            pos = j + 3 + tag_len;
        } else {
            depth -= 1;
            if depth == 0 {
                return Some((j, j + 3));
            }
            pos = j + 3;
        }
    }
    None
}

/// Pull last line-start fenced body whose language tag equals `want_tag` (ASCII, case-insensitive).
///
/// CJK-safe: never advances with a fixed byte offset into multi-byte runes.
/// Nested fences: bodies may embed ```text diagrams; extraction nest-counts
/// line-start fences so the outer fence is not closed early.
pub fn extract_tagged_fence(text: &str, want_tag: &str) -> Option<String> {
    let want = want_tag.trim();
    if want.is_empty() {
        return None;
    }
    let mut search = text;
    let mut best: Option<String> = None;
    while let Some(idx) = search.find("```") {
        // Only treat line-start ``` as a fence opener (skip mid-line triple-backticks).
        if !is_line_start_fence(search, idx) {
            search = &search[idx + 3..];
            continue;
        }
        // ``` is ASCII; idx and idx+3 are always char boundaries.
        let after = &search[idx + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        if tag.eq_ignore_ascii_case(want) {
            let body = after[tag_len..]
                .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if let Some((end, cont)) = close_fence_body(body) {
                let block = body[..end].trim();
                if !block.is_empty() {
                    best = Some(block.to_string());
                }
                search = &body[cont..];
            } else {
                // Unclosed target fence — stop; keep last complete if any.
                break;
            }
        } else {
            // Other fence (plain / markdown / rust / …). Skip this opener;
            // jump past its closer (nesting-aware so ``` inside markdown fences is fine).
            let body = after[tag_len..]
                .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if let Some((_end, cont)) = close_fence_body(body) {
                search = &body[cont..];
            } else if let Some(end) = after.find("```") {
                // fallback: naive skip so we do not stall on odd shapes
                search = &after[end + 3..];
            } else {
                search = after;
            }
        }
    }
    best
}

/// Pull ```plan … ``` body (last fence wins).
pub fn extract_plan_fence(text: &str) -> Option<String> {
    extract_tagged_fence(text, "plan")
}

/// Pull ```session-digest … ``` body (last fence wins).
pub fn extract_session_digest_fence(text: &str) -> Option<String> {
    extract_tagged_fence(text, "session-digest")
}

/// Remove all complete line-start ```session-digest fences from assistant prose
/// (host stores digest on the session; UI reply stays human-first).
pub fn strip_session_digest_fences(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    loop {
        let Some(idx) = rest.find("```") else {
            out.push_str(rest);
            break;
        };
        if !is_line_start_fence(rest, idx) {
            out.push_str(&rest[..idx + 3]);
            rest = &rest[idx + 3..];
            continue;
        }
        let after = &rest[idx + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        let body = after[tag_len..]
            .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
        if tag.eq_ignore_ascii_case("session-digest") {
            out.push_str(&rest[..idx]);
            if let Some((_end, cont)) = close_fence_body(body) {
                rest = body[cont..].trim_start_matches(['\n', '\r']);
            } else {
                // Unclosed digest fence: drop it and stop.
                break;
            }
            continue;
        }
        // Keep other fences: copy from opener through nested-aware closer.
        if let Some((_end, cont)) = close_fence_body(body) {
            let prefix_len = rest.len() - body.len();
            let end = prefix_len + cont;
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        } else {
            out.push_str(rest);
            break;
        }
    }
    // Collapse runs of blank lines left by removals; keep single trailing trim.
    let collapsed = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    let mut cleaned = String::new();
    let mut blank = 0usize;
    for line in collapsed.lines() {
        if line.is_empty() {
            blank += 1;
            if blank <= 1 {
                cleaned.push('\n');
            }
        } else {
            blank = 0;
            if !cleaned.is_empty() && !cleaned.ends_with('\n') {
                cleaned.push('\n');
            }
            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }
    cleaned.trim().to_string()
}
