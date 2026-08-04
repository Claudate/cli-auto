//! Cost-aware CLI catalog · tier · select · escalate (P0/P1 primitives).
//!
//! Apply orchestration lives in [`super::cost_apply`]. Intent in [`super::cost_intent`].
//!
//! [INPUT]: role · available provider names · unhealthy · catalog
//! [OUTPUT]: CostPick · CostRouteReport types · escalate peer name
//! [POS]: domain/worker — pure; no registry / preflight / RunState
//! [PROTOCOL]: never auto-pick fake/sdk
//!   See docs/cost-aware-cli-router-2026-07-27.md

use crate::domain::plan::TaskRole;

use super::types::ProviderId;

/// Quality / cost band for worker CLIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostTier {
    Cheap = 0,
    Mid = 1,
    Flagship = 2,
}

impl CostTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cheap => "cheap",
            Self::Mid => "mid",
            Self::Flagship => "flagship",
        }
    }

    pub fn next_up(self) -> Option<Self> {
        match self {
            Self::Cheap => Some(Self::Mid),
            Self::Mid => Some(Self::Flagship),
            Self::Flagship => None,
        }
    }

    pub fn iter_up_from(self) -> impl Iterator<Item = CostTier> {
        let mut t = Some(self);
        std::iter::from_fn(move || {
            let cur = t?;
            t = cur.next_up();
            Some(cur)
        })
    }
}

/// One auto-routable engine with relative cost (lower = cheaper).
#[derive(Debug, Clone, Copy)]
pub struct ProviderCostEntry {
    pub id: &'static str,
    pub tier: CostTier,
    /// Ordinal within catalog; lower sorts first inside a tier.
    pub cost_rank: u32,
}

/// Built-in catalog (P0 static). fake/sdk omitted on purpose.
pub fn default_cost_catalog() -> &'static [ProviderCostEntry] {
    // rank gaps leave room for future inserts without reshuffling semantics.
    &[
        ProviderCostEntry {
            id: "deepseek",
            tier: CostTier::Cheap,
            cost_rank: 10,
        },
        ProviderCostEntry {
            id: "qwen",
            tier: CostTier::Cheap,
            cost_rank: 20,
        },
        ProviderCostEntry {
            id: "gemini",
            tier: CostTier::Cheap,
            cost_rank: 30,
        },
        ProviderCostEntry {
            id: "kimi",
            tier: CostTier::Cheap,
            cost_rank: 40,
        },
        ProviderCostEntry {
            id: "codebuddy",
            tier: CostTier::Cheap,
            cost_rank: 50,
        },
        ProviderCostEntry {
            id: "copilot",
            tier: CostTier::Cheap,
            cost_rank: 60,
        },
        ProviderCostEntry {
            id: "codex",
            tier: CostTier::Mid,
            cost_rank: 100,
        },
        ProviderCostEntry {
            id: "claude",
            tier: CostTier::Flagship,
            cost_rank: 200,
        },
    ]
}

/// Role → default tier (multi-cli doc aligned; empty role ≈ implement).
pub fn role_default_tier(role: Option<TaskRole>) -> CostTier {
    match role {
        Some(TaskRole::Scout) => CostTier::Cheap,
        Some(TaskRole::Implement) | None => CostTier::Mid,
        Some(TaskRole::Closeout) => CostTier::Mid,
        Some(TaskRole::Integrate) | Some(TaskRole::Inspect) => CostTier::Flagship,
    }
}

/// True for engines that must never be chosen by auto cost / escalate.
pub fn is_non_auto_provider(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "fake" | "mock" | "sdk" | "claude-sdk" | "claude_sdk" | ""
    )
}

fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

fn entry_for<'a>(catalog: &'a [ProviderCostEntry], name: &str) -> Option<&'a ProviderCostEntry> {
    let n = norm(name);
    catalog.iter().find(|e| e.id == n)
}

/// Tier of a catalog entry (pub for P2 budget/sticky helpers).
pub fn entry_tier(catalog: &[ProviderCostEntry], name: &str) -> Option<CostTier> {
    entry_for(catalog, name).map(|e| e.tier)
}

/// Cheapest available provider in `tier`, then higher tiers if the pool is empty.
///
/// `available` = registered (ideally preflight-ok) names. `unhealthy` = open circuit.
pub fn select_in_tier(
    tier: CostTier,
    available: &[String],
    unhealthy: &[String],
    catalog: &[ProviderCostEntry],
) -> Option<CostPick> {
    let avail: Vec<String> = available.iter().map(|s| norm(s)).collect();
    let bad: Vec<String> = unhealthy.iter().map(|s| norm(s)).collect();
    for t in tier.iter_up_from() {
        let mut candidates: Vec<&ProviderCostEntry> = catalog
            .iter()
            .filter(|e| e.tier == t)
            .filter(|e| !is_non_auto_provider(e.id))
            .filter(|e| avail.iter().any(|a| a == e.id))
            .filter(|e| !bad.iter().any(|b| b == e.id))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        candidates.sort_by_key(|e| e.cost_rank);
        let best = candidates[0];
        return Some(CostPick {
            provider: best.id.to_string(),
            tier: best.tier,
            requested_tier: tier,
            cost_rank: best.cost_rank,
            borrowed_up: best.tier > tier,
            sticky: false,
            budget_clamped: false,
        });
    }
    None
}

/// One selection result (pure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostPick {
    pub provider: String,
    pub tier: CostTier,
    pub requested_tier: CostTier,
    pub cost_rank: u32,
    /// True when no engine in requested tier was available.
    pub borrowed_up: bool,
    /// P2: reused a cohort peer's CLI.
    pub sticky: bool,
    /// P2: tier was lowered by run budget ceiling.
    pub budget_clamped: bool,
}

impl CostPick {
    /// Human one-liner for desk / route_note (no engine jargon dump).
    pub fn rationale_zh(&self) -> String {
        let band = match self.tier {
            CostTier::Cheap => "较低费用",
            CostTier::Mid => "中等费用",
            CostTier::Flagship => "高能力",
        };
        let mut parts: Vec<String> = Vec::new();
        if self.sticky {
            parts.push(format!("同波沿用（{}）", band));
        } else if self.borrowed_up {
            parts.push(format!(
                "{}档无人可用，上借到{}（{}）",
                self.requested_tier.as_str(),
                self.tier.as_str(),
                band
            ));
        } else {
            parts.push(format!("{}（{}）", self.tier.as_str(), band));
        }
        if self.budget_clamped {
            parts.push("预算收紧".into());
        }
        format!("费用优选·{}", parts.join("·"))
    }
}

/// Per-task rewrite from [`apply_cost_aware_routing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostRouteChange {
    pub task_id: String,
    pub from: String,
    pub to: String,
    pub tier: CostTier,
    pub rationale: String,
}

/// Report for app provenance stamping (domain never writes RunState).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostRouteReport {
    pub changed: Vec<CostRouteChange>,
    /// still-default but no available pick — left as soft-fill.
    pub skipped_ids: Vec<String>,
}

impl CostRouteReport {
    pub fn changed_ids(&self) -> Vec<String> {
        self.changed.iter().map(|c| c.task_id.clone()).collect()
    }

    /// Desk one-liner, e.g.「实现用 Codex（较低费用向）；巡检用 Claude」.
    pub fn summary_line(&self) -> Option<String> {
        if self.changed.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for c in &self.changed {
            let label = match c.to.as_str() {
                "claude" => "Claude",
                "codex" => "Codex",
                "gemini" => "Gemini",
                "qwen" => "Qwen",
                "kimi" => "Kimi",
                "deepseek" => "CodeWhale",
                "copilot" => "Copilot",
                "codebuddy" => "CodeBuddy",
                other => other,
            };
            let band = match c.tier {
                CostTier::Cheap => "较低费用",
                CostTier::Mid => "中等费用",
                CostTier::Flagship => "高能力",
            };
            parts.push(format!("{}→{}（{}）", c.task_id, label, band));
        }
        Some(format!("费用优选：{}", parts.join("；")))
    }
}

/// P1: next higher-cost auto peer after a failure (tier-up, then cost_rank up).
///
/// Walks catalog entries strictly more expensive than `current`, preferring the
/// cheapest among those still available. Skips tried / unhealthy / non-auto.
pub fn next_escalate_target(
    current: &str,
    available: &[String],
    unhealthy: &[String],
    already_tried: &[String],
    catalog: &[ProviderCostEntry],
) -> Option<String> {
    let cur = norm(current);
    if is_non_auto_provider(&cur) {
        return None;
    }
    let cur_rank = entry_for(catalog, &cur).map(|e| e.cost_rank).unwrap_or(0);
    let avail: Vec<String> = available.iter().map(|s| norm(s)).collect();
    let bad: Vec<String> = unhealthy
        .iter()
        .chain(already_tried.iter())
        .map(|s| norm(s))
        .filter(|s| !s.is_empty())
        .collect();
    let mut better: Vec<&ProviderCostEntry> = catalog
        .iter()
        .filter(|e| e.cost_rank > cur_rank)
        .filter(|e| e.id != cur)
        .filter(|e| !is_non_auto_provider(e.id))
        .filter(|e| avail.iter().any(|a| a == e.id))
        .filter(|e| !bad.iter().any(|b| b == e.id))
        .collect();
    better.sort_by_key(|e| e.cost_rank);
    better.first().map(|e| e.id.to_string())
}

/// Tier of a known provider; unknown → None (caller may fall back to H4 order).
pub fn provider_tier(name: &str) -> Option<CostTier> {
    entry_for(default_cost_catalog(), name).map(|e| e.tier)
}

/// Convenience: known production ids that appear in the default catalog.
pub fn catalog_provider_ids() -> Vec<&'static str> {
    default_cost_catalog().iter().map(|e| e.id).collect()
}

/// Filter a registry name list down to auto-eligible production engines.
pub fn filter_auto_available(names: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    names
        .into_iter()
        .map(|s| norm(s.as_ref()))
        .filter(|s| !is_non_auto_provider(s))
        .filter(|s| {
            ProviderId::parse(s).is_some() || entry_for(default_cost_catalog(), s).is_some()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_tiers_match_product_table() {
        assert_eq!(role_default_tier(Some(TaskRole::Scout)), CostTier::Cheap);
        assert_eq!(role_default_tier(Some(TaskRole::Implement)), CostTier::Mid);
        assert_eq!(role_default_tier(None), CostTier::Mid);
        assert_eq!(
            role_default_tier(Some(TaskRole::Inspect)),
            CostTier::Flagship
        );
        assert_eq!(
            role_default_tier(Some(TaskRole::Integrate)),
            CostTier::Flagship
        );
        assert_eq!(role_default_tier(Some(TaskRole::Closeout)), CostTier::Mid);
    }

    #[test]
    fn select_cheapest_in_cheap_pool() {
        let avail = vec!["gemini".into(), "qwen".into(), "claude".into()];
        let pick = select_in_tier(CostTier::Cheap, &avail, &[], default_cost_catalog()).unwrap();
        assert_eq!(pick.provider, "qwen"); // rank 20 < gemini 30
        assert!(!pick.borrowed_up);
    }

    #[test]
    fn select_borrows_up_when_tier_empty() {
        let avail = vec!["claude".into()];
        let pick = select_in_tier(CostTier::Cheap, &avail, &[], default_cost_catalog()).unwrap();
        assert_eq!(pick.provider, "claude");
        assert!(pick.borrowed_up);
        assert_eq!(pick.tier, CostTier::Flagship);
    }

    #[test]
    fn escalate_from_codex_to_claude() {
        let avail = vec!["codex".into(), "claude".into(), "gemini".into()];
        let next = next_escalate_target("codex", &avail, &[], &[], default_cost_catalog());
        assert_eq!(next.as_deref(), Some("claude"));
    }

    #[test]
    fn escalate_skips_tried_and_unhealthy() {
        let avail = vec!["qwen".into(), "codex".into(), "claude".into()];
        let next = next_escalate_target(
            "qwen",
            &avail,
            &["codex".into()],
            &[],
            default_cost_catalog(),
        );
        assert_eq!(next.as_deref(), Some("claude"));
    }

    #[test]
    fn escalate_none_at_flagship() {
        let avail = vec!["claude".into()];
        assert_eq!(
            next_escalate_target("claude", &avail, &[], &[], default_cost_catalog()),
            None
        );
    }

    #[test]
    fn never_auto_fake() {
        let avail = vec!["fake".into(), "claude".into()];
        let pick = select_in_tier(CostTier::Flagship, &avail, &[], default_cost_catalog()).unwrap();
        assert_eq!(pick.provider, "claude");
        assert!(is_non_auto_provider("fake"));
    }
}
