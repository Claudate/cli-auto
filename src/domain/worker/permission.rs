//! Permission tier — worker safety band (Harness-aligned · A3bis).
//!
//! Pure model: maps the existing `permission_mode` CLI string space into a
//! 3-level semantic tier that is (a) auditable in events.jsonl and (b) renderable
//! as a human safety label without leaking the technical enum to the main path
//! (rule 23).
//!
//! [INPUT]: permission_mode string (bypassPermissions / acceptEdits / dontAsk / default)
//! [OUTPUT]: PermissionTier + as_str/parse + human label + round-trip mapping
//! [POS]: domain/worker — pure; no IO / no spawn / no RunState
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md；不改 apply_permission_mode soft-fill（规则 13 路由不动）
//!
//! See docs/permission-tier-audit-2026-08-14.md

/// Worker safety band. Harness-aligned names; the on-disk event string is the
/// kebab-case `as_str` form, **not** the Rust identifier.
///
/// Mapping to the existing `permission_mode` space (round-trips via
/// [`PermissionTier::from_permission_mode`] / [`PermissionTier::to_permission_mode`]):
/// - `ReadOnly`       ← dontAsk / default       (writes auto-denied → false Done)
/// - `WorkspaceWrite` ← acceptEdits             (file edits auto-allowed)
/// - `FullAccess`     ← bypassPermissions       (Leaf current default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionTier {
    /// 只读 · 不可写任何文件（dontAsk / default 等价）
    ReadOnly,
    /// 可读写项目文件（acceptEdits 等价 · Harness 安全默认）
    WorkspaceWrite,
    /// 完全访问（bypassPermissions 等价 · Leaf 现默认）
    FullAccess,
}

impl PermissionTier {
    /// Stable kebab-case string written to events.jsonl. Stable across renames.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::FullAccess => "full-access",
        }
    }

    /// Recover from the on-disk `as_str` form. Unknown → None.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "read-only" => Some(Self::ReadOnly),
            "workspace-write" => Some(Self::WorkspaceWrite),
            "full-access" => Some(Self::FullAccess),
            _ => None,
        }
    }

    /// Map an existing `permission_mode` CLI string to a tier.
    /// Unknown / empty → `FullAccess` (conservative: matches Leaf's current default
    /// so events stay interpretable for legacy runs with no recorded mode).
    pub fn from_permission_mode(mode: &str) -> Self {
        match mode.trim() {
            "bypassPermissions" => Self::FullAccess,
            "acceptEdits" => Self::WorkspaceWrite,
            "dontAsk" | "default" => Self::ReadOnly,
            _ => {
                // Unknown / empty: treat as the current Leaf default so the tier
                // projection never silently narrows a previously-allowed run.
                Self::FullAccess
            }
        }
    }

    /// Reverse mapping: tier → canonical `permission_mode` string. Used only for
    /// projection / documentation; `apply_permission_mode` keeps its own soft-fill
    /// and is the authority on what actually spawns (rule 13).
    pub fn to_permission_mode(self) -> &'static str {
        match self {
            Self::ReadOnly => "dontAsk",
            Self::WorkspaceWrite => "acceptEdits",
            Self::FullAccess => "bypassPermissions",
        }
    }

    /// Human safety label for the desktop UI (rule 23: no technical enum on the
    /// main path). The UI renders this directly; the Rust side owns the wording.
    pub fn human_label(self) -> &'static str {
        match self {
            Self::ReadOnly => "受限只读",
            Self::WorkspaceWrite => "可读写项目文件",
            Self::FullAccess => "完全访问",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_as_str_parse() {
        for t in [
            PermissionTier::ReadOnly,
            PermissionTier::WorkspaceWrite,
            PermissionTier::FullAccess,
        ] {
            assert_eq!(PermissionTier::parse(t.as_str()), Some(t));
        }
        assert_eq!(PermissionTier::parse("nonsense"), None);
        assert_eq!(PermissionTier::parse(""), None);
    }

    #[test]
    fn from_mode_maps_known_modes() {
        assert_eq!(
            PermissionTier::from_permission_mode("bypassPermissions"),
            PermissionTier::FullAccess
        );
        assert_eq!(
            PermissionTier::from_permission_mode("acceptEdits"),
            PermissionTier::WorkspaceWrite
        );
        assert_eq!(
            PermissionTier::from_permission_mode("dontAsk"),
            PermissionTier::ReadOnly
        );
        assert_eq!(
            PermissionTier::from_permission_mode("default"),
            PermissionTier::ReadOnly
        );
    }

    #[test]
    fn from_mode_unknown_falls_back_to_full_access() {
        // Legacy runs / empty opts must not silently narrow an allowed run.
        assert_eq!(PermissionTier::from_permission_mode(""), PermissionTier::FullAccess);
        assert_eq!(
            PermissionTier::from_permission_mode("something-new"),
            PermissionTier::FullAccess
        );
    }

    #[test]
    fn to_mode_round_trips_known_modes() {
        // For the modes that actually map, from/to must be inverses.
        for mode in ["bypassPermissions", "acceptEdits", "dontAsk"] {
            let tier = PermissionTier::from_permission_mode(mode);
            assert_eq!(tier.to_permission_mode(), mode);
        }
    }

    #[test]
    fn human_label_present_for_all() {
        for t in [
            PermissionTier::ReadOnly,
            PermissionTier::WorkspaceWrite,
            PermissionTier::FullAccess,
        ] {
            let label = t.human_label();
            assert!(!label.is_empty());
            // No technical tokens on the human path (rule 23).
            assert!(!label.contains("bypass"));
            assert!(!label.contains("Permissions"));
        }
    }
}
