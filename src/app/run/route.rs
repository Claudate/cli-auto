//! Provider route override for CLI `cco run` (A0-R3 / A1-4 · S-run extract).
//!
//! [INPUT]: PlanIR · optional --provider / --force-provider
//! [OUTPUT]: short log line when applied; mutates task providers via domain soft/force
//! [POS]: app::run sub-module; policy pure in domain/worker::apply_route_fill
//! [PROTOCOL]: soft-fill must not overwrite explicit routes; force wipe only with force flag

use crate::domain::worker::{apply_route_fill, RouteFillMode};
use crate::plan::PlanIR;

/// CLI `cco run` provider override: soft-fill vs force wipe (A0-R3 / A1-4).
///
/// | flag | behavior |
/// |------|----------|
/// | none | no change |
/// | `--provider P` | soft-fill via [`RouteFillMode::Soft`] |
/// | `--force-provider P` | force wipe via [`RouteFillMode::Force`] |
///
/// When both are set, force wins. Returns a short log line when applied.
pub fn apply_provider_override(
    ir: &mut PlanIR,
    provider: Option<String>,
    force_provider: Option<String>,
) -> Option<String> {
    if let Some(p) = force_provider {
        return apply_route_fill(ir, &p, RouteFillMode::Force).map(|r| r.summary_line());
    }
    if let Some(p) = provider {
        return apply_route_fill(ir, &p, RouteFillMode::Soft).map(|r| r.summary_line());
    }
    None
}
