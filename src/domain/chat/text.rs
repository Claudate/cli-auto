//! Char-boundary-safe string helpers (CJK never mid-rune cut).

/// Char-count truncate (never mid-rune). Appends `…` when shortened.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max_chars).collect::<String>())
}
