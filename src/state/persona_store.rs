//! Project persona storage: persona_id, clarify_depth, split_grain (P0-B · thin).
//!
//! [INPUT]: Config · project_id · Persona preferences
//! [OUTPUT]: project_pins (reused) with keys "persona"/"depth"/"grain"
//! [POS]: state adapter — SQLite SoT for per-project persona
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md
//!
/// Storage for user-chosen persona preferences per project.
/// These values restore when re-opening the same project.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct ProjectPersona {
    /// Role choice: e.g., "pm" (product manager), "founder", "lead_dev", etc.
    pub persona_id: Option<String>,
    /// Clarify depth: e.g., "soft1" (light), "full_opt" (full optimization).
    pub clarify_depth: Option<String>,
    /// Split grain: e.g., "fine" (detailed tasks), "balanced" (medium granularity).
    pub split_grain: Option<String>,
}

/// Internal pin keys for persona preferences (stored in project_pins table).
mod keys {
    pub const PERSONA: &str = "persona";
    pub const DEPTH: &str = "depth";
    pub const GRAIN: &str = "grain";
}

use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::HashMap;

use crate::config::Config;
use crate::state::project_memory::{
    delete_pin, list_pins, upsert_pin, MAX_PIN_KEY_CHARS, MAX_PIN_VALUE_CHARS,
};

/// Normalize string value (trim + truncate). Returns None if empty.
fn normalize_value(s: &str, max_chars: usize) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Simple truncation by chars (not perfect UTF-8 aware but fine for IDs)
    if s.chars().count() > max_chars {
        Some(s.chars().take(max_chars).collect())
    } else {
        Some(s.to_string())
    }
}

/// Upsert a single persona field via pin infrastructure.
/// Uses existing project_pins table with special keys.
fn set_persona_pin(config: &Config, project_id: &str, key: &str, value: &str) -> Result<()> {
    let pid = project_id.trim();
    if pid.is_empty() {
        bail!("project_id empty");
    }
    let key = key.trim();
    if key.is_empty() || key.len() > MAX_PIN_KEY_CHARS {
        bail!("pin key invalid");
    }
    let value = normalize_value(value, MAX_PIN_VALUE_CHARS)
        .ok_or_else(|| anyhow::anyhow!("pin value empty"))?;

    upsert_pin(config, pid, key, &value)?;
    Ok(())
}

/// Get all three persona pins as a HashMap.
/// Returns empty HashMap if no persona pins exist for this project.
fn get_persona_pins(config: &Config, project_id: &str) -> Result<HashMap<String, String>> {
    let pid = project_id.trim();
    if pid.is_empty() {
        bail!("project_id empty");
    }

    let all_pins = list_pins(config, pid)?;
    let mut result = HashMap::new();

    // Only extract persona-related pins
    for pin in all_pins {
        if [keys::PERSONA, keys::DEPTH, keys::GRAIN].contains(&pin.key.as_str()) {
            result.insert(pin.key, pin.value);
        }
    }

    Ok(result)
}

/// Get persona preferences for a project.
/// Returns None if no persona pins exist (UI should show defaults).
/// Existing projects without persona pins → graceful fallback to None.
pub fn get_project_persona(config: &Config, project_id: &str) -> Result<Option<ProjectPersona>> {
    let pins = get_persona_pins(config, project_id)?;

    let persona_id = pins.get(keys::PERSONA).cloned();
    let clarify_depth = pins.get(keys::DEPTH).cloned();
    let split_grain = pins.get(keys::GRAIN).cloned();

    // If none of the fields are set, return None (UI shows defaults)
    if persona_id.is_none() && clarify_depth.is_none() && split_grain.is_none() {
        return Ok(None);
    }

    Ok(Some(ProjectPersona {
        persona_id,
        clarify_depth,
        split_grain,
    }))
}

/// Set persona preferences for a project.
/// All three fields can be set at once or individually via Option.
/// Passing None for a field deletes it from storage.
pub fn set_project_persona(
    config: &Config,
    project_id: &str,
    persona: &ProjectPersona,
) -> Result<()> {
    let pid = project_id.trim();
    if pid.is_empty() {
        bail!("project_id empty");
    }

    // Delete fields that are None
    if persona.persona_id.is_none() {
        let _ = delete_pin(config, pid, keys::PERSONA);
    }
    if persona.clarify_depth.is_none() {
        let _ = delete_pin(config, pid, keys::DEPTH);
    }
    if persona.split_grain.is_none() {
        let _ = delete_pin(config, pid, keys::GRAIN);
    }

    // Set non-None fields
    if let Some(ref p) = persona.persona_id {
        set_persona_pin(config, pid, keys::PERSONA, p)?;
    }
    if let Some(ref d) = persona.clarify_depth {
        set_persona_pin(config, pid, keys::DEPTH, d)?;
    }
    if let Some(ref g) = persona.split_grain {
        set_persona_pin(config, pid, keys::GRAIN, g)?;
    }

    Ok(())
}

/// Convenience function to set only persona_id.
pub fn set_project_persona_id(config: &Config, project_id: &str, persona_id: &str) -> Result<()> {
    let val = normalize_value(persona_id, MAX_PIN_VALUE_CHARS)
        .ok_or_else(|| anyhow::anyhow!("persona_id empty"))?;
    set_persona_pin(config, project_id, keys::PERSONA, &val)
}

/// Convenience function to set only clarify_depth.
pub fn set_project_clarify_depth(config: &Config, project_id: &str, depth: &str) -> Result<()> {
    let val = normalize_value(depth, MAX_PIN_VALUE_CHARS)
        .ok_or_else(|| anyhow::anyhow!("clarify_depth empty"))?;
    set_persona_pin(config, project_id, keys::DEPTH, &val)
}

/// Convenience function to set only split_grain.
pub fn set_project_split_grain(config: &Config, project_id: &str, grain: &str) -> Result<()> {
    let val = normalize_value(grain, MAX_PIN_VALUE_CHARS)
        .ok_or_else(|| anyhow::anyhow!("split_grain empty"))?;
    set_persona_pin(config, project_id, keys::GRAIN, &val)
}

/// Best-effort helpers for prompt injection (never fail callers).
pub fn try_set_project_persona(config: &Config, project_id: &str, persona: &ProjectPersona) {
    if let Err(e) = set_project_persona(config, project_id, persona) {
        tracing::warn!(error = %e, project_id = %project_id, "set_project_persona failed");
    }
}

pub fn try_get_project_persona(config: &Config, project_id: &str) -> Option<ProjectPersona> {
    match get_project_persona(config, project_id) {
        Ok(Some(p)) => Some(p),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, project_id = %project_id, "get_project_persona failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::sqlite::reset_for_test;
    use tempfile::tempdir;

    fn test_cfg() -> (tempfile::TempDir, Config) {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        (dir, cfg)
    }

    #[test]
    fn persona_round_trip_complete() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-persona";

        // Initially None
        assert!(get_project_persona(&cfg, pid).unwrap().is_none());

        // Set all fields
        let persona = ProjectPersona {
            persona_id: Some("pm".into()),
            clarify_depth: Some("soft1".into()),
            split_grain: Some("fine".into()),
        };
        set_project_persona(&cfg, pid, &persona).unwrap();

        // Verify retrieval
        let retrieved = get_project_persona(&cfg, pid).unwrap().unwrap();
        assert_eq!(retrieved.persona_id, Some("pm".into()));
        assert_eq!(retrieved.clarify_depth, Some("soft1".into()));
        assert_eq!(retrieved.split_grain, Some("fine".into()));

        // Update partial
        let updated = ProjectPersona {
            persona_id: Some("founder".into()),
            clarify_depth: None, // delete this field
            split_grain: Some("fine".into()),
        };
        set_project_persona(&cfg, pid, &updated).unwrap();

        let retrieved = get_project_persona(&cfg, pid).unwrap().unwrap();
        assert_eq!(retrieved.persona_id, Some("founder".into()));
        assert_eq!(retrieved.clarify_depth, None); // deleted
        assert_eq!(retrieved.split_grain, Some("fine".into()));
    }

    #[test]
    fn persona_partial_fields() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-persona-partial";

        // Set only persona_id
        set_project_persona_id(&cfg, pid, "lead_dev").unwrap();
        let p = get_project_persona(&cfg, pid).unwrap().unwrap();
        assert_eq!(p.persona_id, Some("lead_dev".into()));
        assert_eq!(p.clarify_depth, None);
        assert_eq!(p.split_grain, None);

        // Set only depth
        set_project_clarify_depth(&cfg, pid, "full_opt").unwrap();
        let p = get_project_persona(&cfg, pid).unwrap().unwrap();
        assert_eq!(p.persona_id, Some("lead_dev".into()));
        assert_eq!(p.clarify_depth, Some("full_opt".into()));
        assert_eq!(p.split_grain, None);

        // Delete all fields
        set_project_persona(&cfg, pid, &ProjectPersona::default()).unwrap();
        assert!(get_project_persona(&cfg, pid).unwrap().is_none());
    }

    #[test]
    fn persona_truncation_and_validation() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-persona-validate";

        // Test value too long (should be truncated)
        let very_long = "x".repeat(300);
        set_project_persona_id(&cfg, pid, &very_long).unwrap();

        let p = get_project_persona(&cfg, pid).unwrap().unwrap();
        assert!(p.persona_id.unwrap().len() <= MAX_PIN_VALUE_CHARS);

        // Test empty value (should fail)
        assert!(set_project_persona_id(&cfg, pid, "").is_err());
        assert!(set_project_clarify_depth(&cfg, pid, "").is_err());
        assert!(set_project_split_grain(&cfg, pid, "").is_err());
    }

    #[test]
    fn persona_best_effort_helpers() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-persona-be";

        // Try-get on non-existent should return None (not error)
        assert!(try_get_project_persona(&cfg, pid).is_none());

        // Try-set should never panic even with bad input
        try_set_project_persona(&cfg, pid, &ProjectPersona::default());
    }
}
