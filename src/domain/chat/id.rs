//! Session id token rules (filesystem-safe; no path join).

/// Default chat session id (wire / disk compatible).
pub const DEFAULT_SESSION: &str = "default";

/// Sanitize session_id to filesystem-safe token (same rules as session_path stem).
pub fn sanitize_session_id(session_id: &str) -> String {
    let safe: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        DEFAULT_SESSION.to_string()
    } else {
        safe
    }
}
