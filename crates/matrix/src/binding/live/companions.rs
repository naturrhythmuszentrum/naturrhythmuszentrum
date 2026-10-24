// SPDX-License-Identifier: AGPL-3.0-only
//! Exact companion maps and numerical target versions travel with selected readings.

use super::{Group, LiveBindingError, ReadingRegion, admission::charge};
use crate::heap::{identity::Key, records::Meaning};
use crate::{CompanionAdapter, Heap, HeapError, ReadingCoverage};
use nrz_bindings::{Budget, CompanionKind, readings::CompanionReadingKind};
use nrz_scalar::Scalar;

/// Native recipe companions retain their exact target edge and complete numerical map.
#[derive(Debug)]
pub(crate) struct NativeCompanion {
    /// Actual companion occupation.
    pub(crate) key: Key,
    /// Exact output value occupation.
    pub(crate) target: Key,
    /// Original output identity and content revision.
    pub(crate) stamp: crate::ValueStamp,
    /// Immutable companion shape identity.
    pub(crate) schema: usize,
    /// Complete logical companion map.
    pub(crate) layout: crate::Layout,
}

/// Match all original target maps and require disjoint, fully covered parameter storage.
pub(super) fn check<P: Scalar>(
    coverage: &ReadingCoverage<usize>,
    companions: &[CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    let mut occupied = coverage.entries().to_vec();
    for companion in companions {
        charge::<crate::EntryTicket<usize>>(companion.entries().len(), budget)?;
        for target in companion.targets() {
            if !coverage.readings().iter().any(|actual| {
                actual.reading() == target.reading() && actual.entries() == target.entries()
            }) {
                return Err(crate::ReadingAdapterError::Target.into());
            }
        }
        for entry in companion.entries() {
            if occupied.contains(entry) {
                return Err(crate::ReadingAdapterError::Overlap.into());
            }
            occupied.push(entry.clone());
            let value = value(regions, entry)?;
            if matches!(
                companion.companion().kind(),
                CompanionReadingKind::Radius(_)
            ) && value < P::ZERO
            {
                return Err(HeapError::InvalidBound {
                    index: usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?,
                }
                .into());
            }
        }
        check_error(companion, companions, regions)?;
    }
    Ok(())
}

/// Separate numerical error is already enclosed by the declared total for the same exact target.
fn check_error<P: Scalar>(
    companion: &CompanionAdapter<usize>,
    companions: &[CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
) -> Result<(), LiveBindingError> {
    if companion.companion().kind() != CompanionReadingKind::Radius(CompanionKind::NumericalError) {
        return Ok(());
    }
    let total = companions
        .iter()
        .find(|candidate| {
            candidate.companion().kind()
                == CompanionReadingKind::Radius(CompanionKind::DeterministicBound)
                && candidate.companion().targets() == companion.companion().targets()
        })
        .ok_or(crate::ReadingAdapterError::Target)?;
    for (error, bound) in companion.entries().iter().zip(total.entries()) {
        if value(regions, error)? > value(regions, bound)? {
            return Err(crate::ReadingAdapterError::Target.into());
        }
    }
    Ok(())
}

/// Resolve actual supplied parameter storage before any allocation.
pub(super) fn value<P: Scalar>(
    regions: &[ReadingRegion<P>],
    entry: &crate::EntryTicket<usize>,
) -> Result<P, HeapError> {
    regions
        .get(entry.region)
        .and_then(|region| {
            usize::try_from(entry.index)
                .ok()
                .and_then(|index| region.values.get(index))
        })
        .copied()
        .ok_or(HeapError::InvalidLayout)
}

impl Heap {
    /// Read declared companion output parameters while retaining both covariance target maps.
    ///
    /// These are stored coordinates; this method does not execute the companion's exact map.
    /// # Errors
    /// Refuses unrelated companion indices and stale value, parameter or target evidence.
    pub fn reading_companion_entries<P: Scalar + 'static>(
        &self,
        binding: &super::ReadingBinding<P>,
        index: usize,
    ) -> Result<Vec<P>, LiveBindingError> {
        self.validate_reading(binding)?;
        let companion = binding
            .group
            .companions
            .get(index)
            .ok_or(crate::ReadingAdapterError::Target)?;
        if !companion
            .targets()
            .iter()
            .any(|target| target.reading() == binding.reading())
        {
            return Err(crate::ReadingAdapterError::Target.into());
        }
        let mut result = Vec::new();
        for entry in companion.entries() {
            let ticket = binding
                .group
                .tickets
                .get(entry.region)
                .ok_or(HeapError::InvalidLayout)?;
            let view = self.value(crate::Handle::<crate::Value, P>::new(
                ticket.scope,
                ticket.key,
            ))?;
            let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
            let rows = view.region().extent().rows();
            result.push(view.region().get(index % rows, index / rows)?);
        }
        Ok(result)
    }
}

/// Recheck every native edge and storage map retained by a result reading.
pub(super) fn validate_native(group: &Group, heap: &Heap) -> Result<(), HeapError> {
    for companion in &group.native {
        let record = heap.descriptors.get(companion.key)?;
        let target = heap.descriptors.get(companion.target)?;
        let exact_target =
            matches!(target.meaning, Meaning::Value { stamp, .. } if stamp == companion.stamp);
        let exact_edge = matches!(record.meaning, Meaning::Bound { target, stamp } if target == companion.target && stamp == companion.stamp);
        if !exact_target
            || !exact_edge
            || record.schema != companion.schema
            || record.layout != companion.layout
        {
            return Err(HeapError::StaleEvidence);
        }
        let region = heap.owning_region(record)?;
        if region.format
            != (match group.precision {
                nrz_scalar::Precision::F32 => crate::storage::Format::Binary32,
                nrz_scalar::Precision::F64 => crate::storage::Format::Binary64,
            })
        {
            return Err(HeapError::PrecisionMismatch);
        }
        heap.storage.resolve(region)?;
    }
    Ok(())
}

impl Heap {
    /// Replace one selected companion's stored output parameters and invalidate every dependent reading.
    ///
    /// Equal-value writes advance versions too. The replacement is a new declaration;
    /// earlier computation evidence and every old dependent view become unavailable.
    /// # Errors
    /// Refuses stale or unrelated targets, incomplete/nonfinite parameters, negative radii,
    /// numerical errors outside the corresponding total, and allocation or resource failure.
    pub fn replace_reading_companion_entries<P: Scalar + 'static>(
        &mut self,
        binding: &super::ReadingBinding<P>,
        index: usize,
        values: &[P],
        budget: &mut Budget,
    ) -> Result<(), LiveBindingError> {
        self.validate_reading(binding)?;
        let companion = binding
            .companions()
            .get(index)
            .ok_or(crate::ReadingAdapterError::Target)?;
        let targeted = companion
            .targets()
            .iter()
            .any(|target| target.reading() == binding.reading());
        if !targeted || values.len() != companion.entries().len() || values.is_empty() {
            return Err(crate::ReadingAdapterError::Target.into());
        }
        replacement_radii(self, binding, companion, values)?;
        super::write::replace(self, &binding.group, companion.entries(), values, budget)
    }
}

/// Total and numerical channels remain separate declarations about one exact selected target.
fn replacement_radii<P: Scalar + 'static>(
    heap: &Heap,
    binding: &super::ReadingBinding<P>,
    companion: &CompanionAdapter<usize>,
    values: &[P],
) -> Result<(), LiveBindingError> {
    let CompanionReadingKind::Radius(kind) = companion.companion().kind() else {
        return Ok(());
    };
    for (index, value) in values.iter().enumerate() {
        if *value < P::ZERO {
            return Err(HeapError::InvalidBound { index }.into());
        }
    }
    let opposite = match kind {
        CompanionKind::DeterministicBound => CompanionKind::NumericalError,
        CompanionKind::NumericalError => CompanionKind::DeterministicBound,
    };
    let other = binding.companions().iter().position(|candidate| {
        candidate.companion().kind() == CompanionReadingKind::Radius(opposite)
            && candidate.companion().targets() == companion.companion().targets()
    });
    if let Some(index) = other {
        let previous = heap.reading_companion_entries(binding, index)?;
        let invalid = values
            .iter()
            .zip(previous)
            .any(|(value, other)| match kind {
                CompanionKind::DeterministicBound => *value < other,
                CompanionKind::NumericalError => *value > other,
            });
        if invalid {
            return Err(crate::ReadingAdapterError::Target.into());
        }
    } else if kind == CompanionKind::NumericalError {
        return Err(crate::ReadingAdapterError::Target.into());
    }
    Ok(())
}
