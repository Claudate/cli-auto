//! Confirm-screen task patch helpers (role / scope paths).
//!
//! [INPUT]: TaskIR · raw role string · scope path list
//! [OUTPUT]: in-place TaskIR patch · cleaned paths · role parse via domain
//! [POS]: planner 子模块；被 view::update_proposed_task 一行委托
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md
//! note: 不复制 soft-fill；confirm 仍是唯一开跑；role 改 inspect 后由
//!       materialize_role_defaults 补默认 scope/tools（调用方负责）
//!       validate_provider_name 放行全部已知 ProviderId（手动改执行通道不被白名单卡住）

use anyhow::{bail, Result};

use crate::plan::{parse_role_input, TaskIR, TaskRole};

/// Normalize confirm-screen path list: trim, drop empty, dedupe keep order.
pub fn clean_scope_paths(raw: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in raw {
        let p = p.trim().to_string();
        if p.is_empty() {
            continue;
        }
        if !out.iter().any(|x| x == &p) {
            out.push(p);
        }
    }
    out
}

/// Apply optional role patch. `None` arg = leave unchanged.
/// Empty / clear tokens → `task.role = None`.
/// Returns whether the field changed.
pub fn apply_role_patch(task: &mut TaskIR, role: Option<String>) -> Result<bool> {
    let Some(raw) = role else {
        return Ok(false);
    };
    let next = parse_role_input(&raw).map_err(|e| anyhow::anyhow!(e))?;
    if task.role == next {
        return Ok(false);
    }
    task.role = next;
    Ok(true)
}

/// Apply optional writable-scope paths patch. `None` arg = leave unchanged.
/// Empty list clears `scope` when readonly/forbid also empty; otherwise keeps
/// scope with empty `paths` (caller may still have readonly/forbid).
/// Returns whether the field changed.
pub fn apply_scope_paths_patch(task: &mut TaskIR, paths: Option<Vec<String>>) -> Result<bool> {
    let Some(raw) = paths else {
        return Ok(false);
    };
    let clean = clean_scope_paths(raw);
    let prev = task
        .scope
        .as_ref()
        .map(|s| s.paths.clone())
        .unwrap_or_default();
    if prev == clean {
        return Ok(false);
    }
    if clean.is_empty() {
        match task.scope.as_mut() {
            None => {}
            Some(s) if s.readonly.is_empty() && s.forbid.is_empty() => {
                task.scope = None;
            }
            Some(s) => {
                s.paths.clear();
            }
        }
        return Ok(true);
    }
    let mut scope = task.scope.take().unwrap_or_default();
    scope.paths = clean;
    task.scope = Some(scope);
    Ok(true)
}

/// Reject unknown provider names on confirm edit (all known production ids incl. `sdk`).
pub fn validate_provider_name(p: &str) -> Result<String> {
    use crate::domain::worker::ProviderId;
    let p = p.trim().to_ascii_lowercase();
    if p.is_empty() {
        bail!("provider 不能为空");
    }
    let Some(id) = ProviderId::parse(&p) else {
        bail!(
            "不支持的 provider: {p}（可选 claude / codex / fake / sdk / gemini / qwen / kimi / deepseek / copilot / codebuddy）"
        );
    };
    Ok(id.as_str().to_string())
}

/// Human label for role in logs / DTO (snake_case).
pub fn role_wire(role: Option<TaskRole>) -> Option<String> {
    role.map(|r| r.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::TaskRole;

    fn bare_task() -> TaskIR {
        TaskIR {
            id: "t1".into(),
            title: "x".into(),
            depends_on: vec![],
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: "p".into(),
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
        }
    }

    #[test]
    fn role_patch_sets_and_clears() {
        let mut t = bare_task();
        assert!(apply_role_patch(&mut t, Some("implement".into())).unwrap());
        assert_eq!(t.role, Some(TaskRole::Implement));
        assert!(apply_role_patch(&mut t, Some("".into())).unwrap());
        assert_eq!(t.role, None);
        assert!(!apply_role_patch(&mut t, None).unwrap());
        let err = apply_role_patch(&mut t, Some("wizard".into())).unwrap_err();
        assert!(err.to_string().contains("角色"), "{err}");
    }

    #[test]
    fn scope_paths_patch_sets_and_clears() {
        let mut t = bare_task();
        assert!(apply_scope_paths_patch(
            &mut t,
            Some(vec![" src/a/** ".into(), "src/a/**".into(), "".into()])
        )
        .unwrap());
        assert_eq!(
            t.scope.as_ref().map(|s| s.paths.clone()),
            Some(vec!["src/a/**".into()])
        );
        assert!(apply_scope_paths_patch(&mut t, Some(vec![])).unwrap());
        assert!(t.scope.is_none());
    }

    #[test]
    fn validate_provider_allows_sdk() {
        assert_eq!(validate_provider_name("SDK").unwrap(), "sdk");
        assert!(validate_provider_name("other").is_err());
    }

    #[test]
    fn validate_provider_allows_all_shell_print_ids() {
        for (raw, want) in [
            ("claude", "claude"),
            ("codex", "codex"),
            ("fake", "fake"),
            ("sdk", "sdk"),
            ("gemini", "gemini"),
            ("google", "gemini"),
            ("qwen", "qwen"),
            ("kimi", "kimi"),
            ("deepseek", "deepseek"),
            ("copilot", "copilot"),
            ("codebuddy", "codebuddy"),
        ] {
            assert_eq!(validate_provider_name(raw).unwrap(), want, "for {raw}");
        }
        // 别名 / 未知 id 拒绝（防误写；前端只给白名单 id）。
        assert!(validate_provider_name("codewhale").is_err());
        assert!(validate_provider_name("default").is_err());
        assert!(validate_provider_name("other").is_err());
    }
}
