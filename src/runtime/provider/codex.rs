//! Codex CLI adapter — thin shell over [`shell_print::ShellPrintProvider`].
//!
//! [INPUT]: StartCtx · TaskIR · config bin
//! [OUTPUT]: WorkerPort (print/exec only)
//! [POS]: runtime/provider；scope/stream 真源在 shell_print/
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: Codex 无 tool allowlist / --append-system-prompt → 前缀拼进 prompt

use super::shell_print::profiles::CODEX;
use super::shell_print::ShellPrintProvider;

// Scope helpers re-exported for unit tests / multi-cli docs (P1-6).
pub use super::shell_print::{build_scope_prefix, with_scope_prefix};

/// Codex CLI second real provider (print / `codex exec`).
pub type CodexProvider = ShellPrintProvider;

/// Construct a Codex shell-print provider.
pub fn new(bin: impl Into<String>, extra_args: Vec<String>) -> CodexProvider {
    ShellPrintProvider::new(CODEX, bin, extra_args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{TaskIR, TaskScope};
    use crate::ports::worker::WorkerPort;
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
        assert!(prefix
            .contains("Writable whitelist (scope.paths): src/module_a/**, .cco-out/feat-a/**"));
        assert!(prefix.contains("Extra readonly ranges (scope.readonly): docs/**"));
        assert!(prefix.contains("Hard forbid (scope.forbid): src/module_b/**, ~"));
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
        assert!(out.starts_with("CCO scope lock:"));
        assert!(out.contains("\n\nimplement feature X\nCCO_DONE ok"));
    }

    #[test]
    fn with_scope_prefix_empty_prompt_is_just_lock() {
        let work = PathBuf::from("/tmp/empty");
        let out = with_scope_prefix("   ", &work, None);
        assert!(out.contains("CCO scope lock: work ONLY inside `/tmp/empty`"));
        assert!(!out.contains("\n\n"));
    }

    #[test]
    fn validate_task_rejects_bg_does_not_fake_allowed_tools() {
        let p = new("codex", vec![]);
        let mut task = TaskIR {
            id: "t1".into(),
            title: "t1".into(),
            depends_on: vec![],
            group: None,
            provider: "codex".into(),
            mode: "bg".into(),
            prompt: "hello".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec![],
        };
        let err = p.validate_task(&task).unwrap_err().to_string();
        assert!(
            err.contains("does not support mode=bg") || err.contains("bg"),
            "expected bg rejection: {err}"
        );
        task.mode = "print".into();
        assert!(p.validate_task(&task).is_ok());
        let caps = p.capabilities();
        assert!(caps.print);
        assert!(!caps.background, "must not fake Claude bg");
        assert!(!caps.session_resume);
    }
}
