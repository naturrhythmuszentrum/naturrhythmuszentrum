// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical diagonal maps keep propagated total and nominal-target error in separate arrays.

use super::CoordinateError;
use nrz_accuracy::Nonnegative;
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_scalar::{ArithmeticContext, DecodedLiteral, Enclosure, Finite, Scalar};

/// Private numerical arrays; their per-entry meanings are retained by the binding execution record.
#[derive(Clone, Debug)]
pub(super) struct Entries<P: Scalar> {
    /// Finite candidate coordinates.
    pub(super) centers: Vec<P>,
    /// Complete radius, with numerical error covered once.
    pub(super) totals: Vec<P>,
    /// Propagated numerical-only radius for the original nominal target.
    pub(super) errors: Vec<P>,
}

/// Retain a fixed finite numerical schedule and reserve all arrays before evaluation.
pub(super) fn evaluate<P: Scalar>(
    input: &Entries<P>,
    coefficients: &[[DecodedLiteral<P>; 2]],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Entries<P>, CoordinateError> {
    let count = input.centers.len();
    if input.totals.len() != count || input.errors.len() != count || coefficients.len() != count {
        return Err(super::HeapError::LengthMismatch.into());
    }
    let length = u64::try_from(count).map_err(|_| super::HeapError::ExtentOverflow)?;
    budget.work(
        length
            .checked_mul(16)
            .ok_or(super::HeapError::ExtentOverflow)?,
        Location::new("coordinate arithmetic"),
    )?;
    reserve::<P>(count, 6, budget)?;
    let mut result = Entries {
        centers: crate::binding::buffer(count)?,
        totals: crate::binding::buffer(count)?,
        errors: crate::binding::buffer(count)?,
    };
    for (index, pair) in coefficients.iter().enumerate() {
        let value = entry(
            input.centers[index],
            input.totals[index],
            input.errors[index],
            pair,
            context,
        )?;
        result.centers.push(value[0]);
        result.totals.push(value[1]);
        result.errors.push(value[2]);
    }
    Ok(result)
}

/// The old numerical radius is already covered by the total; origins affect only the new discrepancy.
fn entry<P: Scalar>(
    center: P,
    total: P,
    error: P,
    pair: &[DecodedLiteral<P>; 2],
    context: &ArithmeticContext,
) -> Result<[P; 3], CoordinateError> {
    let original = Finite::new(center)?;
    let total = Nonnegative::new(total)?;
    let error = Nonnegative::new(error)?;
    if error.get() > total.get() {
        return Err(CoordinateError::Identity);
    }
    let [scale, offset] = *pair;
    let candidate = Finite::new(scale.center().get() * center + offset.center().get())?;
    let image = scale
        .enclosure()
        .mul(Enclosure::singleton(original), context)?
        .add(offset.enclosure(), context)?;
    let discrepancy = Nonnegative::new(image.displacement_from(candidate, context)?.get())?;
    let magnitude = Nonnegative::new(scale.enclosure().abs().upper().get())?;
    Ok([
        candidate.get(),
        total
            .scale(magnitude, context)?
            .add(discrepancy, context)?
            .get(),
        error
            .scale(magnitude, context)?
            .add(discrepancy, context)?
            .get(),
    ])
}

/// Jointly reserve complete private arrays before exposing any partial numerical result.
pub(super) fn reserve<P: Scalar>(
    count: usize,
    arrays: usize,
    budget: &Budget,
) -> Result<(), CoordinateError> {
    let length = u64::try_from(count).map_err(|_| super::HeapError::ExtentOverflow)?;
    let width = arrays
        .checked_mul(std::mem::size_of::<P>())
        .ok_or(super::HeapError::ExtentOverflow)?;
    let bytes = length
        .checked_mul(u64::try_from(width).map_err(|_| super::HeapError::ExtentOverflow)?)
        .ok_or(super::HeapError::ExtentOverflow)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        bytes,
        Location::new("coordinate arrays"),
    )?;
    Ok(())
}

/// Complete shared metadata and all three retained numerical arrays fit the result allowance.
pub(super) fn output<P: Scalar>(
    count: usize,
    retained: u64,
    budget: &Budget,
) -> Result<u64, CoordinateError> {
    let length = u64::try_from(count).map_err(|_| super::HeapError::ExtentOverflow)?;
    let width = u64::try_from(3 * std::mem::size_of::<P>())
        .map_err(|_| super::HeapError::ExtentOverflow)?;
    let bytes = length
        .checked_mul(width)
        .and_then(|value| value.checked_add(retained))
        .and_then(|value| value.checked_add(256))
        .ok_or(super::HeapError::ExtentOverflow)?;
    budget.check(
        BudgetKind::OutputBytes,
        bytes,
        Location::new("coordinate result"),
    )?;
    Ok(bytes)
}
