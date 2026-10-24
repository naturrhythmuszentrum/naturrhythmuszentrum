// SPDX-License-Identifier: AGPL-3.0-only
//! Complete value and companion selections are checked before any numerical access.

use super::{Error, Heap, HeapError, ReadingBinding};
use crate::CompanionExecution;
use nrz_bindings::{Budget, CompanionKind, readings::CompanionReadingKind};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// An affine reading with every original parameter/error source and ordered companion target.
#[derive(Clone, Debug)]
pub struct ReadingSelection<P: Scalar>(Rc<Plan<P>>);

/// Shared plan owns exact maps and immutable versioned source bindings.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Actual selected source entries, not separately imported logical value coordinates.
    source: ReadingBinding<P>,
    /// Complete selected bound/error/covariance execution inventory.
    companions: Vec<CompanionExecution<P>>,
    /// Explicit bounded value-coefficient import policy.
    literals: LiteralLimits,
    /// Shared retained declaration allowance for nested evaluation.
    bytes: u64,
}

impl<P: Scalar> ReadingSelection<P> {
    /// Seal the selected value and its complete original companion source inventory.
    /// # Errors
    /// Refuses missing/duplicate companions, changed target copies, error without total,
    /// no-storage or native members, and cumulative metadata resource limits.
    pub fn new(
        source: ReadingBinding<P>,
        companions: Vec<CompanionExecution<P>>,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        if !source.group.native.is_empty()
            || source.adapter().representation().kind()
                == nrz_bindings::readings::RepresentationKind::None
        {
            return Err(Error::Identity);
        }
        check(&source, &companions, budget)?;
        let bytes = companions.iter().try_fold(0_u64, |sum, companion| {
            sum.checked_add(companion.retained_payload_bytes())
                .ok_or(HeapError::ExtentOverflow)
        })?;
        let metadata = std::mem::size_of::<Plan<P>>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|value| {
                companions
                    .len()
                    .checked_mul(std::mem::size_of::<CompanionExecution<P>>())
                    .and_then(|slots| value.checked_add(slots))
            })
            .ok_or(HeapError::ExtentOverflow)?;
        let mut bytes = bytes
            .checked_add(u64::try_from(metadata).map_err(|_| HeapError::ExtentOverflow)?)
            .ok_or(HeapError::ExtentOverflow)?;
        // Every supplied plan already retains this selected target's witness envelope.
        if companions.is_empty() {
            let witness = u64::try_from(
                source
                    .adapter()
                    .representation()
                    .witness()
                    .exported_byte_len(),
            )
            .map_err(|_| HeapError::ExtentOverflow)?;
            bytes = bytes
                .checked_add(witness.checked_mul(8).ok_or(HeapError::ExtentOverflow)?)
                .and_then(|value| value.checked_add(512))
                .ok_or(HeapError::ExtentOverflow)?;
        }
        budget.check(
            nrz_bindings::readings::algebra::limits::BudgetKind::OutputBytes,
            bytes,
            super::numeric::AT,
        )?;
        Ok(Self(Rc::new(Plan {
            source,
            companions,
            literals,
            bytes,
        })))
    }
    /// Original value identity, selected copies, exact map and source versions.
    #[must_use]
    pub fn source(&self) -> &ReadingBinding<P> {
        &self.0.source
    }
    /// Every companion's original parameter and error sources and both ordered targets.
    #[must_use]
    pub fn companions(&self) -> &[CompanionExecution<P>] {
        &self.0.companions
    }
    /// Shared exact/source allowance retained beside private numerical arrays.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Value coefficient import policy, independent of each companion's policy.
    pub(super) fn literals(&self) -> LiteralLimits {
        self.0.literals
    }
}

impl<P: Scalar + 'static> ReadingSelection<P> {
    /// Validate every source and target even if a selected map coefficient is zero.
    pub(super) fn validate(&self, heap: &Heap) -> Result<(), Error> {
        heap.validate_reading(self.source())?;
        for companion in self.companions() {
            for binding in companion.sources().iter().chain(companion.targets()) {
                heap.validate_reading(binding)?;
            }
        }
        Ok(())
    }
}

/// Same exact reading in another numerical occupation cannot substitute a selected target.
fn same<P: Scalar>(left: &ReadingBinding<P>, right: &ReadingBinding<P>) -> bool {
    Rc::ptr_eq(&left.group, &right.group) && left.index == right.index
}

/// The full declared inventory is required; no missing edge becomes an independence premise.
fn check<P: Scalar>(
    source: &ReadingBinding<P>,
    companions: &[CompanionExecution<P>],
    budget: &mut Budget,
) -> Result<(), Error> {
    let mut radii = [false; 2];
    for (index, companion) in companions.iter().enumerate() {
        budget.work(1, super::numeric::AT)?;
        if !companion
            .targets()
            .iter()
            .any(|target| same(source, target))
        {
            return Err(Error::Identity);
        }
        if companions[..index]
            .iter()
            .any(|prior| prior.exact().target().node() == companion.exact().target().node())
        {
            return Err(Error::Identity);
        }
        if let CompanionReadingKind::Radius(kind) = companion.exact().target().kind() {
            let slot = match kind {
                CompanionKind::DeterministicBound => 0,
                CompanionKind::NumericalError => 1,
            };
            if radii[slot] {
                return Err(Error::Identity);
            }
            radii[slot] = true;
        }
    }
    if radii[1] && !radii[0] {
        return Err(Error::Identity);
    }
    for required in source.selected().companions() {
        budget.work(1, super::numeric::AT)?;
        if !companions
            .iter()
            .any(|actual| actual.exact().target().node() == required.companion().node())
        {
            return Err(Error::Identity);
        }
    }
    Ok(())
}
