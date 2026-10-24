// SPDX-License-Identifier: AGPL-3.0-only
//! Selected original scalar values and their own total/error parameters are inseparable inputs.

use super::{AT, Heap, LcSpectrum, SpectralError as Error, SpectralValue};
use crate::{Coverage, ReadingBinding};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionReadingKind, LcSpectralLaw,
        algebra::{Rational, readings::AffineMap},
    },
};
use nrz_scalar::Scalar;

/// Every input is the exact original formal slot with its complete physical reference identity.
pub(super) fn check<P: Scalar>(
    law: &LcSpectralLaw,
    inputs: &[ReadingBinding<P>; 2],
    budget: &mut Budget,
) -> Result<(), Error> {
    for ((input, formal), meaning) in inputs
        .iter()
        .zip(law.operator().inputs())
        .zip(law.operator().frame().entries())
    {
        crate::binding::adapters::reading_work(input.reading(), budget)
            .map_err(super::LiveBindingError::from)?;
        let reading = input.reading();
        if reading.symbol() != formal
            || reading.family().context() != law.operator().frame().context()
            || input.adapter().entries().len() != 1
            || !input.group.native.is_empty()
        {
            return Err(Error::Identity);
        }
        let [entry] = reading.entries() else {
            return Err(Error::Identity);
        };
        entry.check(meaning)?;
        if reading.map().coefficients() != [Rational::one()]
            || reading.map().offsets() != [Rational::zero()]
        {
            return Err(Error::Identity);
        }
        companions(input, budget)?;
    }
    Ok(())
}

/// The closed scalar input path consumes each declared radius only through an identity parameter map.
fn companions<P: Scalar>(input: &ReadingBinding<P>, budget: &mut Budget) -> Result<(), Error> {
    let mut seen = [false; 2];
    for companion in input.companions() {
        budget.work(1, AT)?;
        if !companion
            .targets()
            .iter()
            .any(|target| target.reading() == input.reading())
        {
            continue;
        }
        let channel = match companion.companion().kind() {
            CompanionReadingKind::Radius(CompanionKind::DeterministicBound) => 0,
            CompanionReadingKind::Radius(CompanionKind::NumericalError) => 1,
            CompanionReadingKind::Covariance => return Err(Error::Identity),
        };
        let same_target = companion.targets().len() == 1;
        let identity = companion.companion().map() == &AffineMap::identity(1, budget)?;
        if seen[channel] || !same_target || !identity {
            return Err(Error::Identity);
        }
        seen[channel] = true;
    }
    Ok(())
}

/// Each physical source has separate original nominal and deterministic-family coverage.
pub(super) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    plan: &LcSpectrum<P>,
    budget: &mut Budget,
) -> Result<([SpectralValue<P>; 2], [Coverage; 2]), Error> {
    let mut values = [SpectralValue {
        center: P::ZERO,
        total: P::ZERO,
        error: P::ZERO,
    }; 2];
    let mut coverage = [Coverage::RepresentedInputs; 2];
    for (index, input) in plan.inputs().iter().enumerate() {
        budget.work(4, AT)?;
        let entries = heap.reading_entries(input)?;
        let [center] = entries.as_slice() else {
            return Err(Error::Identity);
        };
        let total = parameter(heap, input, CompanionKind::DeterministicBound, budget)?;
        let error =
            parameter(heap, input, CompanionKind::NumericalError, budget)?.unwrap_or(P::ZERO);
        coverage[index] = input.coverage(total.is_some());
        let total = total.unwrap_or(P::ZERO);
        values[index] = SpectralValue {
            center: *center,
            total: if total < error { error } else { total },
            error,
        };
    }
    Ok((values, coverage))
}

/// A companion is read from its actual versioned parameter region, never from an output map's source slot.
fn parameter<P: Scalar + 'static>(
    heap: &Heap,
    input: &ReadingBinding<P>,
    kind: CompanionKind,
    budget: &mut Budget,
) -> Result<Option<P>, Error> {
    for (index, companion) in input.companions().iter().enumerate() {
        budget.work(1, AT)?;
        let target = companion
            .targets()
            .iter()
            .any(|target| target.reading() == input.reading());
        if target && companion.companion().kind() == CompanionReadingKind::Radius(kind) {
            let values = heap.reading_companion_entries(input, index)?;
            let [value] = values.as_slice() else {
                return Err(Error::Identity);
            };
            if *value < P::ZERO {
                return Err(Error::Identity);
            }
            return Ok(Some(*value));
        }
    }
    Ok(None)
}
