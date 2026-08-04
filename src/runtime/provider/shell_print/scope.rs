//! Shared cwd/scope lock prompt prefix for shell CLIs without system-prompt flags.
//!
//! [INPUT]: work_dir · TaskScope
//! [OUTPUT]: prefix string · prompt with prefix
//! [POS]: runtime/provider/shell_print — codex / gemini / qwen / …
//! [PROTOCOL]: 变更时更新 shell_print/mod.rs CLAUDE 行

use std::path::Path;

use crate::plan::TaskScope;

/// Build cwd/scope lock text prepended to the worker prompt.
///
/// CLIs without tool allowlist / `--append-system-prompt` get the same class of
/// constraints Claude gets via system prompt as a **prompt prefix**.
pub fn build_scope_prefix(work_dir: &Path, scope: Option<&TaskScope>) -> String {
    let dir = work_dir.display();
    let mut parts = vec![format!(
        "CCO scope lock: work ONLY inside `{dir}`. Never read, list, search, or write outside this project directory. FORBIDDEN: home (~), Desktop, Documents, Downloads, Pictures, Movies, Music, Photos, and any absolute path not under `{dir}`. Do NOT run `find ~`, `ls ~`, `find /Users`, or any home-wide scan. Prefer relative paths from cwd."
    )];

    if let Some(s) = scope {
        if !s.paths.is_empty() {
            parts.push(format!(
                "Writable whitelist (scope.paths): {}. Do not write outside these globs (relative to project root).",
                s.paths.join(", ")
            ));
        }
        if !s.readonly.is_empty() {
            parts.push(format!(
                "Extra readonly ranges (scope.readonly): {}.",
                s.readonly.join(", ")
            ));
        }
        if !s.forbid.is_empty() {
            parts.push(format!(
                "Hard forbid (scope.forbid): {}. Never read, list, search, or write these paths.",
                s.forbid.join(", ")
            ));
        }
    }

    parts.join("\n")
}

/// Prepend scope lock to the user prompt.
pub fn with_scope_prefix(prompt: &str, work_dir: &Path, scope: Option<&TaskScope>) -> String {
    let prefix = build_scope_prefix(work_dir, scope);
    if prompt.trim().is_empty() {
        prefix
    } else {
        format!("{prefix}\n\n{prompt}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scope_prefix_locks_cwd_and_forbids_home() {
        let work = PathBuf::from("/tmp/proj");
        let prefix = build_scope_prefix(&work, None);
        assert!(
            prefix.contains("CCO scope lock: work ONLY inside `/tmp/proj`"),
            "missing cwd lock: {prefix}"
        );
        assert!(prefix.contains("FORBIDDEN: home (~)"));
        assert!(prefix.contains("Desktop"));
        assert!(prefix.contains("Do NOT run `find ~`"));
        assert!(
            !prefix.contains("scope.paths") && !prefix.contains("Writable whitelist"),
            "no paths → no whitelist line"
        );
        assert!(!prefix.contains("scope.forbid") && !prefix.contains("Hard forbid"));
    }

    #[test]
    fn scope_prefix_includes_paths_whitelist_and_forbid() {
        let work = PathBuf::from("/Users/me/project");
        let scope = TaskScope {
            paths: vec!["src/module_a/**".into(), ".cco-out/feat-a/**".into()],
            readonly: vec!["docs/**".into()],
            forbid: vec!["src/module_b/**".into(), "~".into()],
        };
        let prefix = build_scope_prefix(&work, Some(&scope));
        assert!(prefix.contains("CCO scope lock: work ONLY inside `/Users/me/project`"));
        assert!(
            prefix
                .contains("Writable whitelist (scope.paths): src/module_a/**, .cco-out/feat-a/**"),
            "missing paths whitelist: {prefix}"
        );
        assert!(
            prefix.contains("Extra readonly ranges (scope.readonly): docs/**"),
            "missing readonly: {prefix}"
        );
        assert!(
            prefix.contains("Hard forbid (scope.forbid): src/module_b/**, ~"),
            "missing forbid: {prefix}"
        );
        assert!(prefix.contains("Never read, list, search, or write these paths"));
    }

    #[test]
    fn with_scope_prefix_prepends_lock_before_user_prompt() {
        let work = PathBuf::from("/tmp/app");
        let scope = TaskScope {
            paths: vec!["src/**".into()],
            readonly: vec![],
            forbid: vec!["secrets/**".into()],
        };
        let out = with_scope_prefix("implement feature X\nCCO_DONE ok", &work, Some(&scope));
        assert!(
            out.starts_with("CCO scope lock:"),
            "prefix must lead: {}",
            &out[..out.len().min(80)]
        );
        assert!(out.contains("Writable whitelist (scope.paths): src/**"));
        assert!(out.contains("Hard forbid (scope.forbid): secrets/**"));
        assert!(
            out.contains("\n\nimplement feature X\nCCO_DONE ok"),
            "user prompt must follow blank line"
        );
        let lock_at = out.find("CCO scope lock:").unwrap();
        let body_at = out.find("implement feature X").unwrap();
        assert!(lock_at < body_at);
    }

    #[test]
    fn with_scope_prefix_empty_prompt_is_just_lock() {
        let work = PathBuf::from("/tmp/empty");
        let out = with_scope_prefix("   ", &work, None);
        assert!(out.contains("CCO scope lock: work ONLY inside `/tmp/empty`"));
        assert!(!out.contains("\n\n"));
    }
}
