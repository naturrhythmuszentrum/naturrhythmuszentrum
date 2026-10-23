// SPDX-License-Identifier: AGPL-3.0-only
//! Match the declared universe to complete actual regions under the exclusive heap lease.

use super::{LiveBindingError, ReadingBinding, ReadingRegion};
use crate::{EntryTicket, Heap, HeapError, ReadingCoverage};
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_scalar::Scalar;

/// Shared accounting location for live entry coverage.
pub(super) const AT: Location = Location::new("live reading coverage");

/// Bound every new metadata or payload traversal with the caller's cumulative ledger.
pub(super) fn charge<T>(count: usize, budget: &mut Budget) -> Result<(), LiveBindingError> {
    let count = u64::try_from(count).map_err(|_| HeapError::ExtentOverflow)?;
    let width = u64::try_from(std::mem::size_of::<T>()).map_err(|_| HeapError::ExtentOverflow)?;
    let bytes = count.checked_mul(width).ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::MatrixEntries, count, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(count, AT)?;
    Ok(())
}

/// Neither a caller's opaque universe nor a no-storage reading can omit a float.
pub(super) fn universe<P: Scalar>(
    coverage: &ReadingCoverage<usize>,
    companions: &[crate::CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    budget: &mut Budget,
) -> Result<Vec<super::CopyDiscrepancy>, LiveBindingError> {
    charge::<ReadingRegion<P>>(regions.len(), budget)?;
    let mut actual = Vec::new();
    for (region, payload) in regions.iter().enumerate() {
        charge::<P>(payload.values.len(), budget)?;
        crate::heap::publication::scan(payload.extent, &payload.values, None)?;
        actual
            .try_reserve(payload.values.len())
            .map_err(|_| HeapError::Capacity)?;
        for index in 0..payload.values.len() {
            actual.push(EntryTicket {
                region,
                index: u64::try_from(index).map_err(|_| HeapError::ExtentOverflow)?,
            });
        }
    }
    let mut declared = coverage.entries().to_vec();
    for companion in companions {
        declared.extend_from_slice(companion.entries());
    }
    if actual.len() != declared.len() {
        return Err(crate::ReadingAdapterError::Coverage.into());
    }
    for entry in &actual {
        charge::<EntryTicket<usize>>(declared.len(), budget)?;
        if !declared.contains(entry) {
            return Err(crate::ReadingAdapterError::Coverage.into());
        }
    }
    representations(coverage, companions, regions, budget)
}

/// Expanded numerical families require their supported complete block reconstruction.
fn representations<P: Scalar>(
    coverage: &ReadingCoverage<usize>,
    companions: &[crate::CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    budget: &mut Budget,
) -> Result<Vec<super::CopyDiscrepancy>, LiveBindingError> {
    use nrz_bindings::readings::RepresentationKind;
    let mut copies = Vec::new();
    for adapter in coverage.readings() {
        let family = adapter.reading().family();
        if adapter.representation().kind() == RepresentationKind::ComplexBlock {
            charge::<P>(4, budget)?;
            let values = adapter
                .entries()
                .iter()
                .map(|entry| {
                    regions
                        .get(entry.region)
                        .and_then(|region| {
                            usize::try_from(entry.index)
                                .ok()
                                .and_then(|index| region.values.get(index))
                        })
                        .copied()
                        .ok_or(HeapError::InvalidLayout)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if block(&values).is_err() {
                super::copies::append(&mut copies, adapter, companions, regions, &values, budget)?;
            }
        } else if family.node() != family.root()
            && adapter.representation().kind() != RepresentationKind::None
        {
            let covered = coverage.readings().iter().any(|whole| {
                whole.representation().kind() == RepresentationKind::ComplexBlock
                    && whole.reading().family() == family
                    && adapter
                        .entries()
                        .iter()
                        .all(|entry| whole.entries().contains(entry))
            });
            if !covered {
                return Err(HeapError::MissingCoverage.into());
            }
        }
    }
    Ok(copies)
}

/// Complete Complex block centers retain every repeated and negated represented coordinate.
pub(super) fn block<P: Scalar>(values: &[P]) -> Result<(), HeapError> {
    if values.len() != 4 || values[0] != values[3] || values[1] != -values[2] {
        return Err(HeapError::InvalidLayout);
    }
    Ok(())
}

impl Heap {
    /// Resolve the selected stored coordinates without evaluating the physical map.
    ///
    /// Mixed-map evaluation belongs to coordinate lowering. None returns no floats.
    /// # Errors
    /// Refuses unavailable, foreign, stale or precision/layout-substituted bindings.
    pub fn reading_entries<P: Scalar + 'static>(
        &self,
        binding: &ReadingBinding<P>,
    ) -> Result<Vec<P>, LiveBindingError> {
        self.validate_reading(binding)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(binding.adapter().entries().len())
            .map_err(|_| HeapError::Capacity)?;
        for entry in binding.adapter().entries() {
            let ticket = binding
                .group
                .tickets
                .get(entry.region)
                .ok_or(HeapError::InvalidLayout)?;
            let region = self.value(crate::Handle::<crate::Value, P>::new(
                ticket.scope,
                ticket.key,
            ))?;
            let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
            let rows = region.region().extent().rows();
            values.push(region.region().get(index % rows, index / rows)?);
        }
        Ok(values)
    }

    /// Revalidate actual occupations and every selected entry's version.
    pub(crate) fn validate_reading<P: Scalar + 'static>(
        &self,
        binding: &ReadingBinding<P>,
    ) -> Result<(), LiveBindingError> {
        if binding.group.precision != P::PRECISION {
            return Err(HeapError::PrecisionMismatch.into());
        }
        binding.selected().validate(self)?;
        Ok(())
    }
}
