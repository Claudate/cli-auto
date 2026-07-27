//! P3 heuristic intent classifier (no ML · no external proxy).
//!
//! [INPUT]: TaskRole · title · prompt · tags
//! [OUTPUT]: IntentKind · adjusted CostTier · short reason
//! [POS]: domain/worker — pure; used only when CostRouteOpts.intent
//! [PROTOCOL]: Inspect/Integrate never lowered; never silent-overwrite explicit routes
//!   (caller still gates still-default). See docs/cost-aware-cli-router-2026-07-27.md §P3

use crate::domain::plan::TaskRole;

use super::cost_route::CostTier;

/// Coarse task difficulty for tier nudge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentKind {
    /// Typo / format / docs polish — prefer cheaper.
    Trivial,
    /// Normal work — keep role default.
    Routine,
    /// Architecture / multi-module / security — prefer stronger.
    Hard,
}

/// Classified intent + human reason token (matched keyword or tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentHint {
    pub kind: IntentKind,
    /// Short token for route_note (keyword/tag); empty when Routine.
    pub reason: String,
}

impl IntentHint {
    pub fn routine() -> Self {
        Self {
            kind: IntentKind::Routine,
            reason: String::new(),
        }
    }
}

/// Tags that force hard (case-insensitive).
const HARD_TAGS: &[&str] = &["hard", "arch", "architecture", "critical", "complex", "难", "架构"];
/// Tags that force trivial.
const TRIVIAL_TAGS: &[&str] = &["simple", "trivial", "docs", "chore", "typo", "easy", "简单", "文案"];

/// Substrings → hard (EN + CJK). Checked in title+prompt lowercased.
const HARD_NEEDLES: &[&str] = &[
    "architecture",
    "arch redesign",
    "multi-module",
    "cross-module",
    "distributed",
    "race condition",
    "deadlock",
    "security audit",
    "oauth",
    "cryptograph",
    "migrate schema",
    "data migration",
    "refactor entire",
    "system design",
    "架构",
    "重构",
    "跨模块",
    "分布式",
    "竞态",
    "死锁",
    "安全审计",
    "权限模型",
    "全库迁移",
];

const TRIVIAL_NEEDLES: &[&str] = &[
    "typo",
    "fix typo",
    "format only",
    "formatting",
    "docstring",
    "rename only",
    "readme only",
    "lint fix",
    "polish copy",
    "错别字",
    "文案",
    "格式化",
    "仅注释",
    "改注释",
    "拼写",
    "标点",
];

/// Classify from free text + tags. Hard wins over trivial when both match.
pub fn classify_intent(title: &str, prompt: &str, tags: &[String]) -> IntentHint {
    let tags_l: Vec<String> = tags
        .iter()
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    for t in &tags_l {
        if HARD_TAGS.iter().any(|h| t == h || t.contains(h)) {
            return IntentHint {
                kind: IntentKind::Hard,
                reason: format!("tag:{t}"),
            };
        }
    }
    for t in &tags_l {
        if TRIVIAL_TAGS.iter().any(|h| t == h || t.contains(h)) {
            return IntentHint {
                kind: IntentKind::Trivial,
                reason: format!("tag:{t}"),
            };
        }
    }

    let blob = format!("{title}\n{prompt}").to_ascii_lowercase();
    if let Some(n) = HARD_NEEDLES.iter().find(|n| blob.contains(*n)) {
        return IntentHint {
            kind: IntentKind::Hard,
            reason: (*n).to_string(),
        };
    }
    if let Some(n) = TRIVIAL_NEEDLES.iter().find(|n| blob.contains(*n)) {
        return IntentHint {
            kind: IntentKind::Trivial,
            reason: (*n).to_string(),
        };
    }

    // Very short implement-ish prompts without hard signals → slight trivial bias.
    let compact: String = blob.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.chars().count() > 0 && compact.chars().count() < 48 {
        return IntentHint {
            kind: IntentKind::Trivial,
            reason: "short".into(),
        };
    }

    IntentHint::routine()
}

/// Merge role default tier with intent.
///
/// | role | lock |
/// |------|------|
/// | Inspect / Integrate | always Flagship |
/// | other + Hard | bump one band (Cheap→Mid, Mid→Flagship) |
/// | other + Trivial | drop one band (Flagship→Mid, Mid→Cheap) |
/// | Routine | unchanged |
pub fn apply_intent_to_tier(
    role: Option<TaskRole>,
    role_tier: CostTier,
    hint: &IntentHint,
) -> (CostTier, Option<String>) {
    match role {
        Some(TaskRole::Inspect) | Some(TaskRole::Integrate) => (CostTier::Flagship, None),
        _ => match hint.kind {
            IntentKind::Routine => (role_tier, None),
            IntentKind::Hard => {
                let t = match role_tier {
                    CostTier::Cheap => CostTier::Mid,
                    CostTier::Mid => CostTier::Flagship,
                    CostTier::Flagship => CostTier::Flagship,
                };
                let note = if t != role_tier {
                    Some(format!("意图偏难·{}", hint.reason))
                } else {
                    None
                };
                (t, note)
            }
            IntentKind::Trivial => {
                let t = match role_tier {
                    CostTier::Flagship => CostTier::Mid,
                    CostTier::Mid => CostTier::Cheap,
                    CostTier::Cheap => CostTier::Cheap,
                };
                let note = if t != role_tier {
                    Some(format!("意图偏简·{}", hint.reason))
                } else {
                    None
                };
                (t, note)
            }
        },
    }
}

/// One-shot: classify + merge (no-op path when `enabled` is false).
pub fn effective_tier(
    enabled: bool,
    role: Option<TaskRole>,
    role_tier: CostTier,
    title: &str,
    prompt: &str,
    tags: &[String],
) -> (CostTier, Option<String>) {
    if !enabled {
        return (role_tier, None);
    }
    let hint = classify_intent(title, prompt, tags);
    apply_intent_to_tier(role, role_tier, &hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::TaskRole;

    #[test]
    fn hard_keyword_beats_trivial() {
        let h = classify_intent(
            "fix typo in auth",
            "also redesign architecture of session store",
            &[],
        );
        assert_eq!(h.kind, IntentKind::Hard);
    }

    #[test]
    fn trivial_typo() {
        let h = classify_intent("fix typo in README", "错别字 only", &[]);
        assert_eq!(h.kind, IntentKind::Trivial);
    }

    #[test]
    fn tag_hard() {
        let h = classify_intent("work", "do stuff", &["critical".into()]);
        assert_eq!(h.kind, IntentKind::Hard);
        assert!(h.reason.contains("critical"));
    }

    #[test]
    fn tag_simple() {
        let h = classify_intent("work", "do stuff", &["docs".into()]);
        assert_eq!(h.kind, IntentKind::Trivial);
    }

    #[test]
    fn inspect_never_lowered() {
        let hint = IntentHint {
            kind: IntentKind::Trivial,
            reason: "typo".into(),
        };
        let (t, note) =
            apply_intent_to_tier(Some(TaskRole::Inspect), CostTier::Flagship, &hint);
        assert_eq!(t, CostTier::Flagship);
        assert!(note.is_none());
    }

    #[test]
    fn implement_hard_bumps_to_flagship() {
        let hint = IntentHint {
            kind: IntentKind::Hard,
            reason: "架构".into(),
        };
        let (t, note) =
            apply_intent_to_tier(Some(TaskRole::Implement), CostTier::Mid, &hint);
        assert_eq!(t, CostTier::Flagship);
        assert!(note.unwrap().contains("偏难"));
    }

    #[test]
    fn implement_trivial_drops_to_cheap() {
        let hint = IntentHint {
            kind: IntentKind::Trivial,
            reason: "typo".into(),
        };
        let (t, note) =
            apply_intent_to_tier(Some(TaskRole::Implement), CostTier::Mid, &hint);
        assert_eq!(t, CostTier::Cheap);
        assert!(note.unwrap().contains("偏简"));
    }

    #[test]
    fn disabled_effective_keeps_role() {
        let (t, n) = effective_tier(
            false,
            Some(TaskRole::Implement),
            CostTier::Mid,
            "fix typo",
            "x",
            &[],
        );
        assert_eq!(t, CostTier::Mid);
        assert!(n.is_none());
    }

    #[test]
    fn short_prompt_trivial() {
        let h = classify_intent("ok", "hi", &[]);
        assert_eq!(h.kind, IntentKind::Trivial);
        assert_eq!(h.reason, "short");
    }
}
