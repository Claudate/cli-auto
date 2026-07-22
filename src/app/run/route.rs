//! Provider route override for CLI `cco run` (A0-R3 / A1-4 · S-run extract).
//!
//! [INPUT]: PlanIR · optional --provider / --force-provider
//! [OUTPUT]: RouteFillReport when applied (ids for provenance + summary_line)
//! [POS]: app::run sub-module; policy pure in domain/worker::apply_route_fill
//! [PROTOCOL]: soft-fill must not overwrite explicit routes; force wipe only with force flag

use crate::domain::worker::{apply_route_fill, RouteFillMode, RouteFillReport};
use crate::plan::PlanIR;

/// CLI `cco run` provider override: soft-fill vs force wipe (A0-R3 / A1-4).
///
/// | flag | behavior |
/// |------|----------|
/// | none | no change |
/// | `--provider P` | soft-fill via [`RouteFillMode::Soft`] |
/// | `--force-provider P` | force wipe via [`RouteFillMode::Force`] |
///
/// When both are set, force wins. Returns the domain [`RouteFillReport`] when applied
/// (callers stamp `route_source` via [`super::provenance::stamp_route_fill`]).
pub fn apply_provider_override(
    ir: &mut PlanIR,
    provider: Option<String>,
    force_provider: Option<String>,
) -> Option<RouteFillReport> {
    if let Some(p) = force_provider {
        return apply_route_fill(ir, &p, RouteFillMode::Force);
    }
    if let Some(p) = provider {
        return apply_route_fill(ir, &p, RouteFillMode::Soft);
    }
    None
}
