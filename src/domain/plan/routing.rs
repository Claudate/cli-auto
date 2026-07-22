//! Soft tag → provider routing (pure; never overrides explicit route).
//!
//! [INPUT]: PlanIR mut
//! [OUTPUT]: apply_tag_routing → rewritten task ids (for `route_source=tag_routing`)
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部；不写 RunState / 路径

use super::types::PlanIR;

/// First-match tag → provider (same rules as [`apply_tag_routing`]).
///
/// Pure helper for provenance: when a task's current provider equals the
/// tag-implied target, app may stamp `route_source=tag_routing` (last write).
pub fn tag_implied_provider(tags: &[String]) -> Option<&'static str> {
    let tags_lower: Vec<String> = tags
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if tags_lower
        .iter()
        .any(|x| x == "codex" || x == "gpt" || x == "openai")
    {
        return Some("codex");
    }
    if tags_lower
        .iter()
        .any(|x| x == "claude" || x == "anthropic")
    {
        return Some("claude");
    }
    if tags_lower.iter().any(|x| x == "fake" || x == "mock") {
        return Some("fake");
    }
    None
}

/// P2-4 L1 routing: map free-form `tags` (and inspect role) to provider when the
/// task still carries the plan default / empty / `"default"`.
///
/// Rules (first match wins, case-insensitive tags):
/// - tag `codex` | `gpt` | `openai` → `codex`
/// - tag `claude` | `anthropic` → `claude`
/// - tag `fake` | `mock` → `fake`
/// - tag `inspect` or `role: inspect` → keep current provider (inspect defaults
///   handle tools/scope; do not force codex)
///
/// Does **not** rewrite tasks that already declare a concrete non-default engine.
///
/// Returns the task ids whose `provider` was rewritten (for P1-2 provenance).
pub fn apply_tag_routing(plan: &mut PlanIR) -> Vec<String> {
    let default = plan.default_provider.clone();
    let mut rewritten = Vec::new();
    for t in &mut plan.tasks {
        let p = t.provider.trim();
        let still_default = p.is_empty()
            || p.eq_ignore_ascii_case("default")
            || (!default.is_empty() && p.eq_ignore_ascii_case(&default));
        if !still_default {
            continue;
        }
        if let Some(target) = tag_implied_provider(&t.tags) {
            t.provider = target.into();
            rewritten.push(t.id.clone());
            continue;
        }
        // Title/tag soft hint for inspect does not change provider here.
        let _ = t.role;
    }
    rewritten
}

