// SPDX-License-Identifier: AGPL-3.0-only
//! Resolve complete native targets and injective writes before copying any numerical array.

use super::{AT, count};
use crate::binding::live::{LiveBindingError, ReadingBinding};
use crate::heap::{identity::Key, records::Meaning};
use crate::{Heap, HeapError, ValueStamp, ValueVersion};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::Scalar;

/// One existing native value and its complete deterministic companion set.
#[derive(Debug)]
pub(super) struct Bundle {
    /// Value, total and numerical-error occupations in fixed channel order.
    pub(super) keys: [Key; 3],
    /// Selected logical base positions, with no repeated write destination.
    pub(super) indices: Vec<usize>,
    /// Original base identity with its checked successor revision.
    pub(super) stamp: ValueStamp,
}

/// Validate every supplied declaration before constructing a replacement plan.
pub(super) fn check<P: Scalar>(
    binding: &ReadingBinding<P>,
    channels: [&[P]; 3],
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    let entries = binding.adapter().entries();
    if entries.is_empty() || channels.iter().any(|values| values.len() != entries.len()) {
        return Err(HeapError::LengthMismatch.into());
    }
    if binding.group.native.is_empty()
        || !binding.group.companions.is_empty()
        || binding.reading().family().node() != binding.reading().family().root()
    {
        return Err(HeapError::MissingCoverage.into());
    }
    budget.work(
        count(entries.len())?
            .checked_mul(3)
            .ok_or(HeapError::ExtentOverflow)?,
        AT,
    )?;
    for (index, ((&center, &total), &error)) in channels[0]
        .iter()
        .zip(channels[1])
        .zip(channels[2])
        .enumerate()
    {
        if !center.is_finite() {
            return Err(HeapError::NonFinite { index }.into());
        }
        let invalid_total = !total.is_finite() || total < P::ZERO;
        let invalid_error = !error.is_finite() || error < P::ZERO || error > total;
        if invalid_total || invalid_error {
            return Err(HeapError::InvalidBound { index }.into());
        }
    }
    Ok(())
}

/// Reserve the complete metadata schedule before its first allocation or nested query.
pub(super) fn prepare<P: Scalar + 'static>(
    heap: &Heap,
    binding: &ReadingBinding<P>,
    budget: &mut Budget,
) -> Result<Vec<Bundle>, LiveBindingError> {
    let length = count(binding.adapter().entries().len())?;
    let width = count(std::mem::size_of::<Bundle>() + std::mem::size_of::<usize>())?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        length.checked_mul(width).ok_or(HeapError::ExtentOverflow)?,
        AT,
    )?;
    budget.work(
        length
            .checked_mul(length)
            .ok_or(HeapError::ExtentOverflow)?,
        AT,
    )?;
    let mut result: Vec<Bundle> = Vec::new();
    for entry in binding.adapter().entries() {
        let ticket = binding
            .group
            .tickets
            .get(entry.region)
            .ok_or(HeapError::InvalidLayout)?;
        let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
        let position =
            if let Some(position) = result.iter().position(|item| item.keys[0] == ticket.key) {
                position
            } else {
                result
                    .try_reserve_exact(1)
                    .map_err(|_| HeapError::Capacity)?;
                result.push(bundle::<P>(heap, ticket, binding)?);
                result.len() - 1
            };
        let item = result.get_mut(position).ok_or(HeapError::InvalidLayout)?;
        if item.indices.contains(&index) {
            return Err(HeapError::OutputOverlap.into());
        }
        for key in item.keys {
            heap.symbols
                .cover(key)?
                .versions
                .get(index)
                .ok_or(HeapError::InvalidLayout)?
                .get()
                .checked_add(1)
                .ok_or(HeapError::CounterExhausted)?;
        }
        item.indices
            .try_reserve_exact(1)
            .map_err(|_| HeapError::Capacity)?;
        item.indices.push(index);
    }
    Ok(result)
}

/// The current reverse edges and the retained initial native inventory must name the same targets.
fn bundle<P: Scalar + 'static>(
    heap: &Heap,
    ticket: &crate::binding::live::RegionTicket,
    binding: &ReadingBinding<P>,
) -> Result<Bundle, LiveBindingError> {
    let handle = crate::Handle::<crate::Value, P>::new(ticket.scope, ticket.key);
    let view = heap.value(handle)?;
    let record = heap.record(handle)?;
    let Meaning::Value {
        stamp,
        bound: Some(total),
        evaluation: Some(error),
        contributions: None,
        ..
    } = record.meaning
    else {
        return Err(HeapError::MissingCoverage.into());
    };
    for key in [total, error] {
        if !binding.group.native.iter().any(|companion| {
            companion.key == key && companion.target == ticket.key && companion.stamp == stamp
        }) {
            return Err(HeapError::MissingCoverage.into());
        }
        let companion =
            heap.companion(crate::Handle::<crate::Companion, P>::new(heap.scope, key))?;
        if companion.region().layout() != view.region().layout() {
            return Err(HeapError::InvalidLayout.into());
        }
    }
    Ok(Bundle {
        keys: [ticket.key, total, error],
        indices: Vec::new(),
        stamp: ValueStamp {
            id: stamp.id(),
            version: ValueVersion(
                stamp
                    .version()
                    .number()
                    .checked_add(1)
                    .ok_or(HeapError::CounterExhausted)?,
            ),
        },
    })
}
