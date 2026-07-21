//! Soft tag → provider routing (pure; never overrides explicit route).
//!
//! [INPUT]: PlanIR mut
//! [OUTPUT]: apply_tag_routing
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部

use super::types::PlanIR;

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
pub fn apply_tag_routing(plan: &mut PlanIR) {
    let default = plan.default_provider.clone();
    for t in &mut plan.tasks {
        let p = t.provider.trim();
        let still_default = p.is_empty()
            || p.eq_ignore_ascii_case("default")
            || (!default.is_empty() && p.eq_ignore_ascii_case(&default));
        if !still_default {
            continue;
        }
        let tags_lower: Vec<String> = t
            .tags
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if tags_lower.iter().any(|x| x == "codex" || x == "gpt" || x == "openai") {
            t.provider = "codex".into();
            continue;
        }
        if tags_lower.iter().any(|x| x == "claude" || x == "anthropic") {
            t.provider = "claude".into();
            continue;
        }
        if tags_lower.iter().any(|x| x == "fake" || x == "mock") {
            t.provider = "fake".into();
            continue;
        }
        // Title/tag soft hint for inspect does not change provider here.
        let _ = t.role;
    }
}

