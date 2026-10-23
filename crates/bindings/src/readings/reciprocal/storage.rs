// SPDX-License-Identifier: AGPL-3.0-only
//! Retained certificates use their actual canonical lengths before metered export.

use super::{CoordinateNormalization, ReadingError, ReciprocalMapDefinition, support};
use nrz_cas::{
    Cas,
    limits::{Budget, BudgetKind},
    proof::VerifiedWitness,
    readings::ReuseKey,
};

/// Bound the input, exact coefficient and temporary handle arrays before construction.
pub(super) fn proposal(
    frame: &CoordinateNormalization,
    proposed: &ReciprocalMapDefinition,
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    let count = proposed
        .inputs
        .len()
        .checked_add(proposed.map.coefficients().len())
        .and_then(|value| value.checked_add(proposed.map.offsets().len()))
        .ok_or(ReadingError::Shape)?;
    let mut bytes = u64::try_from(count)
        .map_err(|_| ReadingError::Shape)?
        .checked_mul(256)
        .and_then(|value| value.checked_add(1024))
        .ok_or(ReadingError::Shape)?;
    for value in proposed
        .map
        .coefficients()
        .iter()
        .chain(proposed.map.offsets())
    {
        budget.work(1, support::AT)?;
        let (top, bottom) = value.component_bits();
        bytes = bytes
            .checked_add(
                top.div_ceil(64)
                    .saturating_add(bottom.div_ceil(64))
                    .saturating_mul(16),
            )
            .ok_or(ReadingError::Shape)?;
    }
    for proof in [frame.inverse().witness(), frame.inverse().nonsingularity()] {
        bytes = bytes
            .checked_add(
                u64::try_from(proof.exported_byte_len())
                    .map_err(|_| ReadingError::Shape)?
                    .checked_mul(8)
                    .ok_or(ReadingError::Shape)?,
            )
            .ok_or(ReadingError::Shape)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    Ok(bytes)
}

/// Reserve the retained native witness separately from its temporary exported byte copy.
pub(super) fn witness(
    cas: &Cas,
    witness: &VerifiedWitness,
    retained: u64,
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    let size = u64::try_from(witness.exported_byte_len()).map_err(|_| ReadingError::Shape)?;
    let bytes = retained
        .checked_add(size.checked_mul(8).ok_or(ReadingError::Shape)?)
        .and_then(|value| value.checked_add(1024))
        .ok_or(ReadingError::Shape)?;
    budget.with_retained_payload(bytes, support::AT, |budget| {
        cas.export_witness(witness, budget)
    })?;
    Ok(bytes)
}

/// The complete identity remains live alongside its certificate and exact recipe.
pub(super) fn result(retained: u64, key: &ReuseKey, budget: &Budget) -> Result<u64, ReadingError> {
    let bytes = retained
        .checked_add(u64::try_from(key.as_bytes().len()).map_err(|_| ReadingError::Shape)?)
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
    Ok(bytes)
}
