//! Human risk class for split desk / confirm (display only).
//!
//! [INPUT]: task id · role · scope paths · verify_cmd · kind
//! [OUTPUT]: RiskClass + human labels (no IO · no permission_mode)
//! [POS]: domain/plan
//! [PROTOCOL]: 展示层；不改 spawn / confirm 闸；变更时更新 domain/CLAUDE.md
//!
//! Product: PM-facing labels (会改本地 / 会跑命令 / 会外发) instead of
//! engine strings like `bypassPermissions`.

use super::system_ids::{
    is_system_closeout_task, is_system_post_task, SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID,
    SYS_POST_OPEN_PR_ID,
};
use super::types::{TaskRole, SYS_CLOSEOUT_ID};
use super::verify::is_runnable_verify;

/// Coarse action risk for desk chips (not Claude permission_mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskClass {
    /// Plan / inspect / scout — no business write expected.
    Read,
    /// Implement / default do — may edit local paths.
    WriteLocal,
    /// Has host-runnable verify_cmd or similar local shell.
    Exec,
    /// Push / open-PR / leave the machine.
    External,
}

impl RiskClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::WriteLocal => "write_local",
            Self::Exec => "exec",
            Self::External => "external",
        }
    }

    /// Short chip label (拆分台).
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Read => "只读",
            Self::WriteLocal => "改本地",
            Self::Exec => "跑命令",
            Self::External => "会外发",
        }
    }

    /// One-line hint for title / confirm strip.
    pub fn hint_zh(self) -> &'static str {
        match self {
            Self::Read => "只读计划/仓库，不改业务文件",
            Self::WriteLocal => "会改本地代码或文件",
            Self::Exec => "会在本机跑检查命令",
            Self::External => "会推送或发到远端",
        }
    }
}

/// Tag `browser` (case-insensitive) — worker may get browser MCP when config enables it.
pub fn task_has_browser_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| t.trim().eq_ignore_ascii_case("browser"))
}

/// Tag `scrape` — outbound fetch intent for desk risk chip.
pub fn task_has_scrape_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| t.trim().eq_ignore_ascii_case("scrape"))
}

/// Tag `ui-verify` — screenshot / page verify (needs local preview when require_preview).
pub fn task_has_ui_verify_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| t.trim().eq_ignore_ascii_case("ui-verify"))
}

/// Tag `ui-smoke` — form / main-path smoke.
pub fn task_has_ui_smoke_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| t.trim().eq_ignore_ascii_case("ui-smoke"))
}

/// Derive risk from task identity + fields (pure).
///
/// Precedence: External > Exec > role/scope Read|WriteLocal.
///
/// `tags`: optional task tags (`browser` / `scrape` → external when scrape).
pub fn classify_task_risk(
    id: &str,
    role: Option<TaskRole>,
    scope_paths: &[String],
    scope_readonly_only: bool,
    verify_cmd: Option<&str>,
    kind: Option<&str>,
) -> RiskClass {
    classify_task_risk_with_tags(id, role, scope_paths, scope_readonly_only, verify_cmd, kind, &[])
}

/// Same as [`classify_task_risk`] with tags (scrape → External).
pub fn classify_task_risk_with_tags(
    id: &str,
    role: Option<TaskRole>,
    scope_paths: &[String],
    scope_readonly_only: bool,
    verify_cmd: Option<&str>,
    kind: Option<&str>,
    tags: &[String],
) -> RiskClass {
    let id = id.trim();
    if id == SYS_POST_GIT_PUSH_ID || id == SYS_POST_OPEN_PR_ID {
        return RiskClass::External;
    }
    // Outbound scrape / browser fetch of external pages.
    if task_has_scrape_tag(tags) {
        return RiskClass::External;
    }
    // Host shell after Done — show exec even on otherwise-local tasks.
    if verify_cmd
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some_and(is_runnable_verify)
    {
        return RiskClass::Exec;
    }
    // Local browser verify/smoke without scrape: treat like exec (drives Chrome).
    if task_has_browser_tag(tags) {
        return RiskClass::Exec;
    }
    if id == SYS_POST_INSPECT_ID
        || id == SYS_CLOSEOUT_ID
        || is_system_post_task(id)
        || is_system_closeout_task(id)
    {
        // inspect / closeout: read business tree (closeout writes docs only — still not EXTERNAL).
        return match role {
            Some(TaskRole::Closeout) => RiskClass::WriteLocal, // docs/ledger only
            _ => RiskClass::Read,
        };
    }
    if matches!(role, Some(TaskRole::Inspect) | Some(TaskRole::Scout)) {
        return RiskClass::Read;
    }
    if kind.map(|k| k.eq_ignore_ascii_case("check")).unwrap_or(false)
        && scope_paths.is_empty()
        && scope_readonly_only
    {
        return RiskClass::Read;
    }
    if scope_readonly_only && scope_paths.is_empty() {
        // No writable paths declared — still default implement = local write intent.
        if matches!(role, Some(TaskRole::Implement) | Some(TaskRole::Integrate))
            || kind.map(|k| k.eq_ignore_ascii_case("do")).unwrap_or(true)
        {
            return RiskClass::WriteLocal;
        }
        return RiskClass::Read;
    }
    if !scope_paths.is_empty() && scope_readonly_only {
        return RiskClass::Read;
    }
    RiskClass::WriteLocal
}

/// Wire helpers for PlanTaskView (no TaskRole required on caller).
pub fn classify_task_risk_wire(
    id: &str,
    role_wire: Option<&str>,
    scope_paths: &[String],
    scope_readonly: &[String],
    has_writable_scope: bool,
    verify_cmd: Option<&str>,
    kind: Option<&str>,
) -> RiskClass {
    classify_task_risk_wire_with_tags(
        id,
        role_wire,
        scope_paths,
        scope_readonly,
        has_writable_scope,
        verify_cmd,
        kind,
        &[],
    )
}

/// Wire helper with tags (cco-split / PlanIR task tags).
pub fn classify_task_risk_wire_with_tags(
    id: &str,
    role_wire: Option<&str>,
    scope_paths: &[String],
    scope_readonly: &[String],
    has_writable_scope: bool,
    verify_cmd: Option<&str>,
    kind: Option<&str>,
    tags: &[String],
) -> RiskClass {
    let role = role_wire.and_then(TaskRole::parse);
    let scope_readonly_only = !has_writable_scope;
    let paths = if has_writable_scope {
        scope_paths
    } else if !scope_readonly.is_empty() {
        scope_readonly
    } else {
        &[][..]
    };
    classify_task_risk_with_tags(
        id,
        role,
        paths,
        scope_readonly_only,
        verify_cmd,
        kind,
        tags,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pr_are_external() {
        assert_eq!(
            classify_task_risk(SYS_POST_GIT_PUSH_ID, None, &[], false, None, Some("system")),
            RiskClass::External
        );
        assert_eq!(
            classify_task_risk(SYS_POST_OPEN_PR_ID, None, &[], false, None, Some("system")),
            RiskClass::External
        );
    }

    #[test]
    fn inspect_is_read() {
        assert_eq!(
            classify_task_risk(
                SYS_POST_INSPECT_ID,
                Some(TaskRole::Inspect),
                &[],
                true,
                None,
                Some("check")
            ),
            RiskClass::Read
        );
    }

    #[test]
    fn verify_cmd_promotes_exec() {
        assert_eq!(
            classify_task_risk(
                "t1",
                Some(TaskRole::Implement),
                &["web/**".into()],
                false,
                Some("npm test"),
                Some("do")
            ),
            RiskClass::Exec
        );
    }

    #[test]
    fn implement_with_paths_is_write_local() {
        assert_eq!(
            classify_task_risk(
                "t1",
                Some(TaskRole::Implement),
                &["src/app/**".into()],
                false,
                None,
                Some("do")
            ),
            RiskClass::WriteLocal
        );
    }

    #[test]
    fn labels_are_human() {
        assert_eq!(RiskClass::External.label_zh(), "会外发");
        assert_eq!(RiskClass::WriteLocal.hint_zh().contains("本地"), true);
    }

    #[test]
    fn scrape_tag_is_external() {
        assert_eq!(
            classify_task_risk_with_tags(
                "scrape-1",
                Some(TaskRole::Implement),
                &["content/**".into()],
                false,
                None,
                Some("do"),
                &["browser".into(), "scrape".into()],
            ),
            RiskClass::External
        );
    }

    #[test]
    fn browser_ui_verify_is_exec() {
        assert_eq!(
            classify_task_risk_with_tags(
                "ui-1",
                Some(TaskRole::Implement),
                &[".cco-out/browser/**".into()],
                false,
                None,
                Some("do"),
                &["browser".into(), "ui-verify".into()],
            ),
            RiskClass::Exec
        );
    }

    #[test]
    fn ui_verify_and_smoke_tag_helpers() {
        assert!(task_has_ui_verify_tag(&["ui-verify".into()]));
        assert!(task_has_ui_smoke_tag(&["UI-SMOKE".into()]));
        assert!(!task_has_ui_verify_tag(&["browser".into()]));
    }
}
