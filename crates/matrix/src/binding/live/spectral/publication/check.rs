// SPDX-License-Identifier: AGPL-3.0-only
//! Exact output inventory and every companion target precede numerical publication.

use super::{AT, Error};
use crate::{CompanionAdapter, EntryTicket, HeapError, ReadingCoverage};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionReadingKind, LcSpectralResultDefinition, RepresentationKind,
        algebra::{Rational, limits::BudgetKind},
    },
};

/// Six declared views use one value coordinate, with no numerical occupation for exact real zeros.
pub(super) fn coverage(
    definition: &LcSpectralResultDefinition,
    coverage: &ReadingCoverage<usize>,
    companions: &[CompanionAdapter<usize>],
    budget: &mut Budget,
) -> Result<(), Error> {
    let entry = EntryTicket {
        region: 0,
        index: 0,
    };
    if coverage.entries() != std::slice::from_ref(&entry) || coverage.readings().len() != 6 {
        return Err(Error::Identity);
    }
    for (adapter, expected) in coverage.readings().iter().zip(definition.outputs()) {
        crate::binding::adapters::reading_work(adapter.reading(), budget)
            .map_err(crate::LiveBindingError::from)?;
        if adapter.reading() != expected.reading()
            || adapter.representation().kind() != expected.kind()
            || adapter.representation().reconstruction() != expected.reconstruction()
        {
            return Err(Error::Identity);
        }
        let entries = if expected.kind() == RepresentationKind::None {
            &[][..]
        } else {
            std::slice::from_ref(&entry)
        };
        if adapter.entries() != entries {
            return Err(Error::Identity);
        }
    }
    parameters(definition, companions, budget)
}

/// Every radius is attached to its own exact target and map, including each logical zero row.
fn parameters(
    definition: &LcSpectralResultDefinition,
    companions: &[CompanionAdapter<usize>],
    budget: &mut Budget,
) -> Result<(), Error> {
    if companions.len() != 8 {
        return Err(Error::Identity);
    }
    let mut seen = [[false; 6]; 2];
    for companion in companions {
        budget.work(6, AT)?;
        let [target] = companion.targets() else {
            return Err(Error::Identity);
        };
        let index = definition
            .outputs()
            .iter()
            .position(|output| output.reading() == target.reading())
            .ok_or(Error::Identity)?;
        if index == 2 || index == 3 {
            return Err(Error::Identity);
        }
        let channel = match companion.companion().kind() {
            CompanionReadingKind::Radius(CompanionKind::DeterministicBound) => 0,
            CompanionReadingKind::Radius(CompanionKind::NumericalError) => 1,
            CompanionReadingKind::Covariance => return Err(Error::Identity),
        };
        let (offset, count) = match index {
            0 => (0, 2),
            1 => (2, 2),
            4 => (4, 1),
            5 => (5, 1),
            _ => return Err(Error::Identity),
        };
        let expected: Vec<_> = (offset..offset + count)
            .map(|index| EntryTicket {
                region: channel + 1,
                index,
            })
            .collect();
        if seen[channel][index] || companion.entries() != expected {
            return Err(Error::Identity);
        }
        let map = companion.companion().map();
        let coefficients = if index < 2 {
            vec![Rational::zero(), Rational::one()]
        } else {
            vec![Rational::one()]
        };
        if map.inputs() != 1
            || map.coefficients() != coefficients
            || map.offsets().iter().any(|value| !value.is_zero())
        {
            return Err(Error::Identity);
        }
        seen[channel][index] = true;
    }
    Ok(())
}

/// Charge every newly owned adapter and overlap proof; shared plans and definitions retain their owners.
pub(super) fn storage<P: nrz_scalar::Scalar>(
    coverage: &ReadingCoverage<usize>,
    companions: &[CompanionAdapter<usize>],
    sources: usize,
    budget: &mut Budget,
) -> Result<u64, Error> {
    use nrz_bindings::readings::ReadingRelation;
    let mut bytes = sized(
        1,
        std::mem::size_of::<super::Plan<P>>() + 2 * std::mem::size_of::<usize>(),
    )?;
    add(
        &mut bytes,
        sized(
            coverage.entries().len(),
            std::mem::size_of::<EntryTicket<usize>>(),
        )?,
    )?;
    for adapter in coverage
        .readings()
        .iter()
        .chain(companions.iter().flat_map(CompanionAdapter::targets))
    {
        budget.work(1, AT)?;
        add(
            &mut bytes,
            sized(1, std::mem::size_of::<crate::ReadingAdapter<usize>>())?,
        )?;
        add(
            &mut bytes,
            sized(
                adapter.entries().len(),
                std::mem::size_of::<EntryTicket<usize>>(),
            )?,
        )?;
        // Native witness envelopes also bound each owned rational map and selection buffer.
        add(
            &mut bytes,
            sized(adapter.representation().witness().exported_byte_len(), 8)?,
        )?;
    }
    for relation in coverage.overlaps() {
        budget.work(1, AT)?;
        add(
            &mut bytes,
            sized(1, std::mem::size_of::<ReadingRelation>())?,
        )?;
        add(
            &mut bytes,
            sized(relation.witness().exported_byte_len(), 8)?,
        )?;
    }
    for companion in companions {
        budget.work(1, AT)?;
        add(&mut bytes, companion_storage(companion)?)?;
    }
    add(
        &mut bytes,
        sized(
            sources,
            std::mem::size_of::<crate::binding::live::selected::SelectedReading>(),
        )?,
    )?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// Record vectors and exact buffer allowances use checked integer sizes.
fn sized(count: usize, width: usize) -> Result<u64, Error> {
    u64::try_from(count.checked_mul(width).ok_or(HeapError::ExtentOverflow)?)
        .map_err(|_| HeapError::ExtentOverflow.into())
}

/// A larger metadata owner cannot wrap into a smaller budget charge.
fn add(total: &mut u64, value: u64) -> Result<(), Error> {
    *total = total.checked_add(value).ok_or(HeapError::ExtentOverflow)?;
    Ok(())
}

/// The target witness envelope bounds its closed radius map; these records are separately owned.
fn companion_storage(companion: &CompanionAdapter<usize>) -> Result<u64, Error> {
    let mut bytes = 0;
    add(
        &mut bytes,
        sized(1, std::mem::size_of::<CompanionAdapter<usize>>())?,
    )?;
    add(
        &mut bytes,
        sized(
            companion.entries().len(),
            std::mem::size_of::<EntryTicket<usize>>(),
        )?,
    )?;
    add(
        &mut bytes,
        sized(
            companion.companion().targets().len(),
            std::mem::size_of::<nrz_bindings::readings::PhysicalReading>(),
        )?,
    )?;
    let parameters = companion.companion().parameters();
    let units = parameters
        .units()
        .len()
        .checked_add(companion.companion().units().len())
        .ok_or(HeapError::ExtentOverflow)?;
    add(
        &mut bytes,
        sized(units, std::mem::size_of::<nrz_bindings::PairingWitness>())?,
    )?;
    let target = std::mem::size_of::<nrz_bindings::readings::algebra::Handle>()
        + std::mem::size_of::<Vec<nrz_bindings::readings::EntryMeaning>>()
        + std::mem::size_of::<nrz_bindings::readings::EntryMeaning>();
    add(&mut bytes, sized(parameters.targets().len(), target)?)?;
    // The target's witness envelope covers this closed one-input radius map and unit limbs.
    Ok(bytes)
}

/// Publishing clones the declared metadata and adds the produced bindings, versions and values.
pub(super) fn produced<P: nrz_scalar::Scalar>(plan: &super::Plan<P>) -> Result<u64, Error> {
    let mut bytes = plan.bytes;
    add(
        &mut bytes,
        sized(1, std::mem::size_of::<crate::binding::live::Group>())?,
    )?;
    add(
        &mut bytes,
        sized(1, std::mem::size_of::<super::PublishedLcSpectrum<P>>())?,
    )?;
    add(
        &mut bytes,
        sized(6, std::mem::size_of::<crate::ReadingBinding<P>>())?,
    )?;
    add(
        &mut bytes,
        sized(3, std::mem::size_of::<crate::binding::live::RegionTicket>())?,
    )?;
    add(
        &mut bytes,
        sized(13, std::mem::size_of::<P>() + std::mem::size_of::<u64>())?,
    )?;
    Ok(bytes)
}
