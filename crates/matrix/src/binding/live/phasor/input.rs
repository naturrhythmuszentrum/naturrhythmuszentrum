// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar copy views retain complete source and companion identities without numerical duplication.

use super::{AT, Heap, PhasorError, PhasorOperand, numeric::Values};
use crate::{Coverage, ReadingBinding};
use nrz_bindings::readings::algebra::readings::AffineMap;
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{CompanionReadingKind, RepresentationKind},
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Every copy is an identity view into the same already atomic complete family.
pub(super) fn check<P: Scalar>(
    whole: &ReadingBinding<P>,
    copies: &[ReadingBinding<P>; 4],
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    if whole.adapter().representation().kind() != RepresentationKind::ComplexBlock
        || whole.adapter().entries().len() != 4
        || !whole.group.native.is_empty()
    {
        return Err(PhasorError::Identity);
    }
    for (index, copy) in copies.iter().enumerate() {
        crate::binding::adapters::reading_work(copy.reading(), budget)
            .map_err(super::LiveBindingError::from)?;
        let same_group = Rc::ptr_eq(&whole.group, &copy.group);
        let same_family = whole.reading().family() == copy.reading().family();
        let selected =
            copy.adapter().entries() == std::slice::from_ref(&whole.adapter().entries()[index]);
        if !same_group
            || !same_family
            || !selected
            || copy.reading().map() != &AffineMap::identity(1, budget)?
        {
            return Err(PhasorError::Identity);
        }
        check_companions(copy, budget)?;
    }
    Ok(())
}

/// A stored output parameter is usable only when its exact map is the checked scalar identity.
fn check_companions<P: Scalar>(
    copy: &ReadingBinding<P>,
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    let mut kinds = [false; 2];
    for companion in copy.companions() {
        budget.work(1, AT)?;
        if companion
            .targets()
            .iter()
            .any(|target| target.reading() == copy.reading())
        {
            let kind = match companion.companion().kind() {
                CompanionReadingKind::Radius(CompanionKind::DeterministicBound) => 0,
                CompanionReadingKind::Radius(CompanionKind::NumericalError) => 1,
                CompanionReadingKind::Covariance => return Err(PhasorError::Identity),
            };
            if kinds[kind] || companion.companion().map() != &AffineMap::identity(1, budget)? {
                return Err(PhasorError::Identity);
            }
            kinds[kind] = true;
        }
    }
    Ok(())
}

/// Copy the small fixed arrays only after all original source and error versions are current.
pub(super) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    operand: &PhasorOperand<P>,
    budget: &mut Budget,
) -> Result<(Values<P>, Coverage), PhasorError> {
    let mut result = Values {
        centers: [P::ZERO; 4],
        totals: [P::ZERO; 4],
        errors: [P::ZERO; 4],
    };
    let mut covered = true;
    for (index, copy) in operand.copies().iter().enumerate() {
        budget.work(4, AT)?;
        let values = heap.reading_entries(copy)?;
        let [center] = values.as_slice() else {
            return Err(PhasorError::Identity);
        };
        let total = parameter(heap, copy, CompanionKind::DeterministicBound, budget)?;
        let error =
            parameter(heap, copy, CompanionKind::NumericalError, budget)?.unwrap_or(P::ZERO);
        covered &= total.is_some();
        result.centers[index] = *center;
        result.errors[index] = error;
        let total = total.unwrap_or(P::ZERO);
        result.totals[index] = if total < error { error } else { total };
    }
    let coverage = operand.whole().coverage(covered);
    Ok((result, coverage))
}

/// Read the exact declared scalar output parameter; identity was checked before execution.
fn parameter<P: Scalar + 'static>(
    heap: &Heap,
    copy: &ReadingBinding<P>,
    kind: CompanionKind,
    budget: &mut Budget,
) -> Result<Option<P>, PhasorError> {
    for (index, companion) in copy.companions().iter().enumerate() {
        budget.work(1, AT)?;
        let target = companion
            .targets()
            .iter()
            .any(|target| target.reading() == copy.reading());
        if target && companion.companion().kind() == CompanionReadingKind::Radius(kind) {
            let parameters = heap.reading_companion_entries(copy, index)?;
            let [value] = parameters.as_slice() else {
                return Err(PhasorError::Identity);
            };
            if *value < P::ZERO {
                return Err(PhasorError::Identity);
            }
            return Ok(Some(*value));
        }
    }
    Ok(None)
}
