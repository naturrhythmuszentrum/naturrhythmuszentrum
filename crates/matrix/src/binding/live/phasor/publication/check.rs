// SPDX-License-Identifier: AGPL-3.0-only
//! Closed complete result layout checks bind every produced float to its exact declared interpretation.

use super::super::{AT, HeapError, PhasorError, PhasorProduct};
use crate::{CompanionAdapter, EntryTicket, ReadingAdapter, ReadingCoverage};
use nrz_bindings::readings::algebra::{
    limits::BudgetKind,
    readings::AffineMap,
    syntax::{AtomicSort, Extent, Field, Sort},
};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{CompanionReadingKind, PhasorResultDefinition, RepresentationKind},
};
use nrz_scalar::Scalar;

/// Require the exact whole block, the original vector selection and all four scalar copies.
pub(super) fn coverage(
    definition: &PhasorResultDefinition,
    coverage: &ReadingCoverage<usize>,
    companions: &[CompanionAdapter<usize>],
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    let expected = (0..4)
        .map(|index| EntryTicket { region: 0, index })
        .collect::<Vec<_>>();
    if coverage.entries() != expected {
        return Err(PhasorError::Identity);
    }
    let mut whole = false;
    let mut vector = false;
    let mut copies = [false; 4];
    for adapter in coverage.readings() {
        crate::binding::adapters::reading_work(adapter.reading(), budget)
            .map_err(crate::LiveBindingError::from)?;
        if adapter.reading().family() != definition.output().reading().family() {
            return Err(PhasorError::Identity);
        }
        positions(adapter, budget)?;
        whole |= adapter.reading() == definition.output().reading()
            && adapter.representation().kind() == RepresentationKind::ComplexBlock
            && adapter.representation().reconstruction() == definition.output().reconstruction();
        if let Some(index) = scalar(adapter, budget)? {
            copies[index] = true;
        }
        let sort = Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(2),
            columns: Extent::Literal(1),
        });
        if adapter.reading().sort() == &sort
            && adapter.entries() == &expected[..2]
            && adapter.reading().map() == &AffineMap::identity(2, budget)?
        {
            vector = true;
        }
    }
    if !whole || !vector || copies.iter().any(|present| !present) {
        return Err(PhasorError::Identity);
    }
    parameters(definition, companions, budget)
}

/// Every literal output ticket names its corresponding original selected storage position.
fn positions(adapter: &ReadingAdapter<usize>, budget: &mut Budget) -> Result<(), PhasorError> {
    for (ticket, index) in adapter
        .entries()
        .iter()
        .zip(adapter.reading().selection().indices())
    {
        budget.work(1, AT)?;
        let logical = u64::from(index.row) + 2 * u64::from(index.column);
        if ticket.region != 0 || ticket.index != logical || logical >= 4 {
            return Err(PhasorError::Identity);
        }
    }
    Ok(())
}

/// Only an actual Real scalar identity selects a per-copy numerical parameter.
fn scalar(
    adapter: &ReadingAdapter<usize>,
    budget: &mut Budget,
) -> Result<Option<usize>, PhasorError> {
    let [entry] = adapter.entries() else {
        return Ok(None);
    };
    if adapter.reading().sort() != &Sort::Value(AtomicSort::Scalar(Field::Real))
        || adapter.reading().map() != &AffineMap::identity(1, budget)?
    {
        return Ok(None);
    }
    let index = usize::try_from(entry.index).map_err(|_| HeapError::ExtentOverflow)?;
    if entry.region != 0 || index >= 4 {
        return Err(PhasorError::Identity);
    }
    Ok(Some(index))
}

/// Two complete identity channels attach each produced radius to its own selected copy.
fn parameters(
    definition: &PhasorResultDefinition,
    companions: &[CompanionAdapter<usize>],
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    if companions.len() != 8 {
        return Err(PhasorError::Identity);
    }
    let mut seen = [[false; 4]; 2];
    for companion in companions {
        budget.work(1, AT)?;
        let [target] = companion.targets() else {
            return Err(PhasorError::Identity);
        };
        if target.reading().family() != definition.output().reading().family() {
            return Err(PhasorError::Identity);
        }
        let index = scalar(target, budget)?.ok_or(PhasorError::Identity)?;
        let channel = match companion.companion().kind() {
            CompanionReadingKind::Radius(CompanionKind::DeterministicBound) => 0,
            CompanionReadingKind::Radius(CompanionKind::NumericalError) => 1,
            CompanionReadingKind::Covariance => return Err(PhasorError::Identity),
        };
        let expected = EntryTicket {
            region: channel + 1,
            index: u64::try_from(index).map_err(|_| HeapError::ExtentOverflow)?,
        };
        if seen[channel][index]
            || companion.entries() != std::slice::from_ref(&expected)
            || companion.companion().map() != &AffineMap::identity(1, budget)?
        {
            return Err(PhasorError::Identity);
        }
        seen[channel][index] = true;
    }
    Ok(())
}

/// Retain exact declarations and all ancestors beside the complete numerical product and import.
pub(super) fn storage<P: Scalar>(
    product: &PhasorProduct<P>,
    definition: &PhasorResultDefinition,
    coverage: &ReadingCoverage<usize>,
    sources: usize,
    budget: &mut Budget,
) -> Result<u64, PhasorError> {
    let mut bytes = product
        .retained_payload_bytes()
        .checked_add(definition.retained_payload_bytes())
        .and_then(|size| size.checked_add(8192))
        .ok_or(HeapError::ExtentOverflow)?;
    for adapter in coverage.readings() {
        budget.work(1, AT)?;
        let proof = u64::try_from(adapter.representation().witness().exported_byte_len())
            .map_err(|_| HeapError::ExtentOverflow)?;
        bytes = bytes
            .checked_add(proof.checked_mul(8).ok_or(HeapError::ExtentOverflow)?)
            .ok_or(HeapError::ExtentOverflow)?;
    }
    let ancestors = u64::try_from(sources).map_err(|_| HeapError::ExtentOverflow)?;
    bytes = bytes
        .checked_add(
            ancestors
                .checked_mul(128)
                .ok_or(HeapError::ExtentOverflow)?,
        )
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}
