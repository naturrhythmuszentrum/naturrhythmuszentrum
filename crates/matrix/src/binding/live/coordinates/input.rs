// SPDX-License-Identifier: AGPL-3.0-only
//! Validate the original exact input definition before exposing any stored numerical coordinate.

use super::{CoordinateContract, CoordinateError, ReadingBinding, numeric::Entries};
use crate::{Coverage, Heap};
use nrz_bindings::{
    Budget, CompanionKind, Measure,
    readings::{CompanionReadingKind, algebra::readings::AffineMap},
};
use nrz_scalar::Scalar;

/// Identity inputs preserve the exact interpretation of every actual selected storage copy.
pub(super) fn check<P: Scalar>(
    binding: &ReadingBinding<P>,
    contract: &CoordinateContract,
    budget: &mut Budget,
) -> Result<Vec<usize>, CoordinateError> {
    let reading = binding.reading();
    if reading.family().context() != contract.frame().context() {
        return Err(CoordinateError::Identity);
    }
    contract.frame().check_entries(reading.entries(), budget)?;
    identity(reading.map(), budget)?;
    if reading
        .entries()
        .iter()
        .any(|entry| entry.descriptor().measure() != &Measure::coherent())
    {
        return Err(CoordinateError::UnsupportedReading);
    }
    if !binding.group.native.is_empty() {
        return Err(CoordinateError::UnsupportedCompanion);
    }
    let mut covariance = Vec::new();
    for (index, companion) in binding.companions().iter().enumerate() {
        if companion
            .targets()
            .iter()
            .any(|target| target.reading() == reading)
        {
            if matches!(
                companion.companion().kind(),
                CompanionReadingKind::Radius(_)
            ) {
                identity(companion.companion().map(), budget)
                    .map_err(|_| CoordinateError::UnsupportedCompanion)?;
            } else {
                crate::binding::live::admission::charge::<usize>(covariance.len() + 1, budget)?;
                covariance
                    .try_reserve_exact(1)
                    .map_err(|_| crate::HeapError::Capacity)?;
                covariance.push(index);
            }
        }
    }
    Ok(covariance)
}

/// Nonidentity maps need their own executed image and cannot silently become raw identity data.
fn identity(map: &AffineMap, budget: &mut Budget) -> Result<(), CoordinateError> {
    if map != &AffineMap::identity(map.inputs(), budget)? {
        return Err(CoordinateError::UnsupportedReading);
    }
    Ok(())
}

/// Capture all three arrays after complete metadata admission, preserving total and error separately.
pub(super) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    binding: &ReadingBinding<P>,
    budget: &Budget,
) -> Result<(Entries<P>, Coverage), CoordinateError> {
    let count = binding.reading().entries().len();
    super::numeric::reserve::<P>(count, 3, budget)?;
    let centers = heap.reading_entries(binding)?;
    let total = companion(heap, binding, CompanionKind::DeterministicBound)?;
    let error = companion(heap, binding, CompanionKind::NumericalError)?;
    let coverage = binding.coverage(total.is_some());
    let zeros = || -> Result<Vec<P>, CoordinateError> {
        let mut result = crate::binding::buffer(count)?;
        result.resize(count, P::ZERO);
        Ok(result)
    };
    Ok((
        Entries {
            centers,
            totals: total.map_or_else(zeros, Ok)?,
            errors: error.map_or_else(zeros, Ok)?,
        },
        coverage,
    ))
}

/// Ambiguous duplicate companions refuse instead of choosing by iteration order or equal schema.
fn companion<P: Scalar + 'static>(
    heap: &Heap,
    binding: &ReadingBinding<P>,
    kind: CompanionKind,
) -> Result<Option<Vec<P>>, CoordinateError> {
    let mut index = None;
    for (position, companion) in binding.companions().iter().enumerate() {
        let targeted = companion
            .targets()
            .iter()
            .any(|target| target.reading() == binding.reading());
        if targeted && companion.companion().kind() == CompanionReadingKind::Radius(kind) {
            if index.is_some() {
                return Err(CoordinateError::UnsupportedCompanion);
            }
            index = Some(position);
        }
    }
    index
        .map(|index| {
            heap.reading_companion_entries(binding, index)
                .map_err(CoordinateError::from)
        })
        .transpose()
}
