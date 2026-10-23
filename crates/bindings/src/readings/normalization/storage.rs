// SPDX-License-Identifier: AGPL-3.0-only
//! Retained vectors and exact certificates reduce the allowance of nested CAS work.

use super::super::{EntryMeaning, ReadingError, support};
use nrz_cas::{
    Cas, ExactConstant, Rational,
    limits::{Budget, BudgetKind},
    proof::VerifiedWitness,
    readings::CheckedMapInverse,
};

/// Shared entry meanings are reference counted; the vector owns only its element records.
pub(super) fn entries(values: &[EntryMeaning]) -> Result<u64, ReadingError> {
    bytes(values.len(), std::mem::size_of::<EntryMeaning>())?
        .checked_add(256)
        .ok_or(ReadingError::Shape)
}

/// Exact graph constants share their store; rational values own their bounded integer limbs.
pub(super) fn constants(values: &[ExactConstant]) -> Result<u64, ReadingError> {
    let mut total = bytes(values.len(), std::mem::size_of::<ExactConstant>())?;
    for value in values {
        if let Some(rational) = value.as_rational() {
            total = total
                .checked_add(limbs(rational)?)
                .ok_or(ReadingError::Shape)?;
        }
    }
    Ok(total)
}

/// Bound certificate metadata conservatively from its canonical witness envelope.
pub(super) fn witness(
    cas: &Cas,
    value: &VerifiedWitness,
    retained: u64,
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    // Export itself copies at most the caller's witness ceiling while the original remains live.
    let ceiling = budget.limits().witness_bytes;
    let reserved = retained
        .checked_add(
            ceiling
                .checked_mul(8)
                .and_then(|value| value.checked_add(1024))
                .ok_or(ReadingError::Shape)?,
        )
        .ok_or(ReadingError::Shape)?;
    let encoded = budget.with_retained_payload(reserved, support::AT, |budget| {
        cas.export_witness(value, budget)
    })?;
    let size = u64::try_from(encoded.len()).map_err(|_| ReadingError::Shape)?;
    let total = retained
        .checked_add(size.checked_mul(8).ok_or(ReadingError::Shape)?)
        .and_then(|value| value.checked_add(1024))
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, total, support::AT)?;
    Ok(total)
}

/// Keep determinant and identity certificates together with their native inverse.
pub(super) fn inverse(
    cas: &Cas,
    value: &CheckedMapInverse,
    retained: u64,
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    // CAS bounds the two canonical certificates together by the inverse output allowance.
    let ceiling = budget
        .limits()
        .witness_bytes
        .saturating_mul(2)
        .min(budget.limits().output_bytes);
    let reserved = retained
        .checked_add(
            ceiling
                .checked_mul(8)
                .and_then(|value| value.checked_add(2048))
                .ok_or(ReadingError::Shape)?,
        )
        .ok_or(ReadingError::Shape)?;
    let (identity, nonsingularity) =
        budget.with_retained_payload(reserved, support::AT, |budget| {
            let identity = encoded_size(cas, value.witness(), budget)?;
            let nonsingularity = encoded_size(cas, value.nonsingularity(), budget)?;
            Ok::<_, ReadingError>((identity, nonsingularity))
        })?;
    let total = identity
        .checked_add(nonsingularity)
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| value.checked_add(2048))
        .and_then(|value| value.checked_add(retained))
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, total, support::AT)?;
    budget.check(BudgetKind::OutputBytes, total, support::AT)?;
    Ok(total)
}

/// Integer-safe allocation accounting never traverses an unused rectangle axis.
fn bytes(count: usize, width: usize) -> Result<u64, ReadingError> {
    u64::try_from(count)
        .map_err(|_| ReadingError::Shape)?
        .checked_mul(u64::try_from(width).map_err(|_| ReadingError::Shape)?)
        .ok_or(ReadingError::Shape)
}

/// Reserve both complete vector capacities and their already retained integer limbs.
pub(super) fn pairs(
    scales: &[ExactConstant],
    offsets: &[ExactConstant],
    count: usize,
) -> Result<u64, ReadingError> {
    let spare = count
        .checked_mul(2)
        .and_then(|value| value.checked_sub(scales.len() + offsets.len()))
        .ok_or(ReadingError::Shape)?;
    constants(scales)?
        .checked_add(constants(offsets)?)
        .and_then(|value| {
            value.checked_add(bytes(spare, std::mem::size_of::<ExactConstant>()).ok()?)
        })
        .ok_or(ReadingError::Shape)
}

/// Check the complete owned rational image before its first coefficient clone.
pub(super) fn export(
    scales: &[ExactConstant],
    offsets: &[ExactConstant],
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    let width = scales.len();
    let zeros = width
        .checked_mul(width.saturating_sub(1))
        .ok_or(ReadingError::Shape)?;
    let count = width
        .checked_mul(width)
        .and_then(|value| value.checked_add(width))
        .ok_or(ReadingError::Shape)?;
    let mut total = bytes(count, std::mem::size_of::<Rational>())?;
    total = total
        .checked_add(
            u64::try_from(zeros)
                .map_err(|_| ReadingError::Shape)?
                .checked_mul(limbs(&Rational::zero())?)
                .ok_or(ReadingError::Shape)?,
        )
        .ok_or(ReadingError::Shape)?;
    budget.work(
        u64::try_from(count).map_err(|_| ReadingError::Shape)?,
        support::AT,
    )?;
    for value in scales.iter().chain(offsets) {
        let rational = value
            .as_rational()
            .ok_or(crate::UnitError::UnsupportedExactConstantExport)?;
        total = total
            .checked_add(limbs(rational)?)
            .ok_or(ReadingError::Shape)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, total, support::AT)?;
    budget.check(BudgetKind::OutputBytes, total, support::AT)?;
    Ok(())
}

/// Native limbs and their allocation bookkeeping are bounded independently of decimal spelling.
fn limbs(value: &Rational) -> Result<u64, ReadingError> {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(64)
        .checked_add(denominator.div_ceil(64))
        .and_then(|value| value.checked_mul(16))
        .ok_or(ReadingError::Shape)
}

/// Bound every diagonal clone and every explicit zero before constructing the dense native map.
pub(super) fn diagonal(
    scales: &[ExactConstant],
    offsets: &[ExactConstant],
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    let width = scales.len();
    let zeros = width
        .checked_mul(width.saturating_sub(1))
        .ok_or(ReadingError::Shape)?;
    let zero = ExactConstant::zero();
    let spare = u64::try_from(zeros)
        .map_err(|_| ReadingError::Shape)?
        .checked_mul(constants(std::slice::from_ref(&zero))?)
        .ok_or(ReadingError::Shape)?;
    let total = constants(scales)?
        .checked_add(constants(offsets)?)
        .and_then(|value| value.checked_add(spare))
        .ok_or(ReadingError::Shape)?;
    budget.work(
        u64::try_from(width.checked_mul(width).ok_or(ReadingError::Shape)?)
            .map_err(|_| ReadingError::Shape)?,
        support::AT,
    )?;
    budget.check(BudgetKind::StagedPayloadBytes, total, support::AT)?;
    Ok(total)
}

/// Drop the temporary canonical byte vector before exporting another simultaneously retained witness.
fn encoded_size(
    cas: &Cas,
    value: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<u64, ReadingError> {
    u64::try_from(cas.export_witness(value, budget)?.len()).map_err(|_| ReadingError::Shape)
}
