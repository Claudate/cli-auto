//! Three on-ramps for the clarify phase (vibe-check subset).

use serde::{Deserialize, Serialize};

/// Product on-ramps for the clarify phase (vibe-check subset).
///
/// Chinese product labels:
/// - [`ThinkFirst`]: 想清楚再说
/// - [`IdeaToPlan`]: 从想法到计划（默认）
/// - [`PlanOnly`]: 已想清，直接写计划
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyEntry {
    /// 想清楚再说 — Brief only; plan optional.
    ThinkFirst,
    /// 从想法到计划 — default: clarify → Brief → plan fence.
    #[default]
    IdeaToPlan,
    /// 已想清，直接写计划 — skip grilling; still force min plan chapters.
    PlanOnly,
}

impl ClarifyEntry {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThinkFirst => "think_first",
            Self::IdeaToPlan => "idea_to_plan",
            Self::PlanOnly => "plan_only",
        }
    }

    /// Chinese product label (UI / prompt).
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::ThinkFirst => "想清楚再说",
            Self::IdeaToPlan => "从想法到计划",
            Self::PlanOnly => "已想清，直接写计划",
        }
    }

    /// Parse wire key or common Chinese label. Unknown → None.
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() {
            return None;
        }
        match s {
            "think_first" | "think-first" | "想清楚再说" | "想清楚" => Some(Self::ThinkFirst),
            "idea_to_plan" | "idea-to-plan" | "从想法到计划" | "default" => {
                Some(Self::IdeaToPlan)
            }
            "plan_only"
            | "plan-only"
            | "已想清直接写计划"
            | "已想清，直接写计划"
            | "直接写计划" => Some(Self::PlanOnly),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wire_and_zh_labels() {
        assert_eq!(ClarifyEntry::parse("think_first"), Some(ClarifyEntry::ThinkFirst));
        assert_eq!(ClarifyEntry::parse("想清楚再说"), Some(ClarifyEntry::ThinkFirst));
        assert_eq!(ClarifyEntry::parse("idea_to_plan"), Some(ClarifyEntry::IdeaToPlan));
        assert_eq!(ClarifyEntry::parse("从想法到计划"), Some(ClarifyEntry::IdeaToPlan));
        assert_eq!(ClarifyEntry::parse("plan_only"), Some(ClarifyEntry::PlanOnly));
        assert_eq!(
            ClarifyEntry::parse("已想清，直接写计划"),
            Some(ClarifyEntry::PlanOnly)
        );
        assert_eq!(ClarifyEntry::parse(""), None);
        assert_eq!(ClarifyEntry::parse("unknown"), None);
    }

    #[test]
    fn default_is_idea_to_plan() {
        assert_eq!(ClarifyEntry::default(), ClarifyEntry::IdeaToPlan);
    }
}
