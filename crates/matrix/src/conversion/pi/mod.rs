// SPDX-License-Identifier: AGPL-3.0-only
//! Shared immutable pi regions and explicitly metered certified descriptor lowering.

mod evidence;
mod lower;
mod region;

pub use evidence::PiConstantEvidence;
pub(crate) use lower::CertifiedMap;
pub use lower::PiConversionUse;
pub use region::PiConstant;

use super::ConversionError;
use nrz_bindings::{
    Budget, MapError,
    readings::algebra::{Location, limits::BudgetKind},
};

/// Check logical byte policy without mutating the cumulative ledger.
pub(super) fn check(
    budget: &Budget,
    kind: BudgetKind,
    bytes: usize,
) -> Result<(), ConversionError> {
    let count = u64::try_from(bytes).map_err(|_| ConversionError::ResourceLimit)?;
    budget
        .check(kind, count, Location::new("certified pi lowering"))
        .map_err(MapError::from)?;
    Ok(())
}

/// Commit the complete successful lifecycle Work in one atomic charge.
pub(super) fn charge(budget: &mut Budget, work: u64) -> Result<(), ConversionError> {
    budget
        .work(work, Location::new("certified pi lowering"))
        .map_err(MapError::from)?;
    Ok(())
}
