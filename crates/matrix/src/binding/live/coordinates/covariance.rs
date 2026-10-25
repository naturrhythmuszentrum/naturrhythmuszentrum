// SPDX-License-Identifier: AGPL-3.0-only
//! Covariance remains a declared parameter with two ordered coordinate factors and its own error.

use super::{CoordinateError, CoordinateReading, ReadingBinding};
use crate::ConstantCoefficient;
use crate::{Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// No copied descriptor graph or invented independent source accompanies a parameter array.
#[derive(Debug)]
pub(super) struct Parameter<P: Scalar> {
    /// Original companion position in the source group's immutable inventory.
    pub(super) index: usize,
    /// Covariance parameter centers in logical column order.
    pub(super) centers: Vec<P>,
    /// Numerical-only radius for each represented original parameter.
    pub(super) errors: Vec<P>,
}

/// The source selection already validates every original covariance target and parameter version.
pub(super) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    source: &ReadingBinding<P>,
    indices: &[usize],
    coefficients: &[[ConstantCoefficient<P>; 2]],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Vec<Parameter<P>>, CoordinateError> {
    if indices.is_empty() {
        return Ok(Vec::new());
    }
    let count = indices.iter().try_fold(0_usize, |count, &index| {
        let companion = source
            .companions()
            .get(index)
            .ok_or(CoordinateError::Identity)?;
        count
            .checked_add(companion.entries().len())
            .ok_or(CoordinateError::Heap(HeapError::ExtentOverflow))
    })?;
    reserve::<P>(
        count,
        indices.len(),
        source.reading().entries().len(),
        6,
        budget,
    )?;
    let mut result = crate::binding::buffer(indices.len())?;
    for &index in indices {
        let centers = heap.reading_companion_entries(source, index)?;
        let errors = vec![P::ZERO; centers.len()];
        let input = Parameter {
            index,
            centers,
            errors,
        };
        result.push(evaluate(source, &input, coefficients, context, budget)?);
    }
    Ok(result)
}

/// Restoration reuses both original target identities and propagates prior parameter errors once.
pub(super) fn restore<P: Scalar>(
    input: &CoordinateReading<P>,
    coefficients: &[[ConstantCoefficient<P>; 2]],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Vec<Parameter<P>>, CoordinateError> {
    if input.0.covariance.is_empty() {
        return Ok(Vec::new());
    }
    let count = input
        .0
        .covariance
        .iter()
        .try_fold(0_usize, |count, parameter| {
            count
                .checked_add(parameter.centers.len())
                .ok_or(HeapError::ExtentOverflow)
        })?;
    reserve::<P>(
        count,
        input.0.covariance.len(),
        input.centers().len(),
        3,
        budget,
    )?;
    let mut result = crate::binding::buffer(input.0.covariance.len())?;
    for parameter in &input.0.covariance {
        result.push(evaluate(
            input.source(),
            parameter,
            coefficients,
            context,
            budget,
        )?);
    }
    Ok(result)
}

/// Exactly the axes targeting this coordinate reading transform; every other axis stays physical.
fn evaluate<P: Scalar>(
    source: &ReadingBinding<P>,
    input: &Parameter<P>,
    coefficients: &[[ConstantCoefficient<P>; 2]],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Parameter<P>, CoordinateError> {
    let companion = source
        .companions()
        .get(input.index)
        .ok_or(CoordinateError::Identity)?;
    let [left, right] = companion.targets() else {
        return Err(CoordinateError::Identity);
    };
    let rows = left.reading().entries().len();
    let columns = right.reading().entries().len();
    if rows.checked_mul(columns) != Some(input.centers.len())
        || input.errors.len() != input.centers.len()
    {
        return Err(HeapError::LengthMismatch.into());
    }
    reserve::<P>(input.centers.len(), 1, 0, 0, budget)?;
    budget.work(
        u64::try_from(input.centers.len())
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(12)
            .ok_or(HeapError::ExtentOverflow)?,
        AT,
    )?;
    let mut result = Parameter {
        index: input.index,
        centers: crate::binding::buffer(input.centers.len())?,
        errors: crate::binding::buffer(input.centers.len())?,
    };
    for (index, (&center, &error)) in input.centers.iter().zip(&input.errors).enumerate() {
        let first = factor(
            left.reading() == source.reading(),
            index % rows,
            coefficients,
        )?;
        let second = factor(
            right.reading() == source.reading(),
            index / rows,
            coefficients,
        )?;
        let candidate = Finite::new(center * first.0 * second.0)?;
        let value = Enclosure::singleton(Finite::new(center)?)
            .add(Enclosure::new(-error, error)?, context)?
            .mul(first.1, context)?
            .mul(second.1, context)?;
        result.centers.push(candidate.get());
        result
            .errors
            .push(value.displacement_from(candidate, context)?.get());
    }
    Ok(result)
}

/// Origins never enter covariance; a distinct target is unchanged on its own ordered axis.
fn factor<P: Scalar>(
    matched: bool,
    index: usize,
    coefficients: &[[ConstantCoefficient<P>; 2]],
) -> Result<(P, Enclosure<P>), CoordinateError> {
    if matched {
        let pair = coefficients.get(index).ok_or(CoordinateError::Identity)?;
        Ok((pair[0].center().get(), pair[0].enclosure()))
    } else {
        Ok((P::ONE, Enclosure::singleton(Finite::new(P::ONE)?)))
    }
}

/// All parameter transformation resources use one distinct ledger location.
const AT: Location = Location::new("coordinate covariance");

/// Charge simultaneous input/output channels and parameter metadata before their allocations.
fn reserve<P: Scalar>(
    count: usize,
    parameters: usize,
    coordinates: usize,
    channels: usize,
    budget: &Budget,
) -> Result<(), CoordinateError> {
    budget.check(
        BudgetKind::StagedPayloadBytes,
        staged_bytes::<P>(count, parameters, coordinates, channels)?,
        AT,
    )?;
    Ok(())
}

/// Planning and allocation admission share the same simultaneous parameter/channel extent.
pub(super) fn staged_bytes<P: Scalar>(
    count: usize,
    parameters: usize,
    coordinates: usize,
    channels: usize,
) -> Result<u64, CoordinateError> {
    let values = super::numeric::array_bytes::<P>(coordinates, channels)?;
    bytes::<P>(count, parameters)?
        .checked_mul(2)
        .and_then(|value| value.checked_add(values))
        .ok_or_else(|| HeapError::ExtentOverflow.into())
}

/// Both parameter channels and their fixed metadata remain part of the retained snapshot.
pub(super) fn bytes<P: Scalar>(count: usize, parameters: usize) -> Result<u64, CoordinateError> {
    let entries = count
        .checked_mul(2 * std::mem::size_of::<P>())
        .ok_or(HeapError::ExtentOverflow)?;
    let metadata = parameters
        .checked_mul(std::mem::size_of::<Parameter<P>>())
        .ok_or(HeapError::ExtentOverflow)?;
    u64::try_from(
        entries
            .checked_add(metadata)
            .ok_or(HeapError::ExtentOverflow)?,
    )
    .map_err(|_| HeapError::ExtentOverflow.into())
}

/// Empty covariance inventories preserve the existing deterministic lifecycle allowance.
pub(super) fn output<P: Scalar>(
    parameters: &[Parameter<P>],
    retained: u64,
    budget: &Budget,
) -> Result<u64, CoordinateError> {
    let count = parameters.iter().try_fold(0_usize, |count, parameter| {
        count
            .checked_add(parameter.centers.len())
            .ok_or(HeapError::ExtentOverflow)
    })?;
    let result = retained
        .checked_add(bytes::<P>(count, parameters.len())?)
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::OutputBytes, result, AT)?;
    Ok(result)
}
