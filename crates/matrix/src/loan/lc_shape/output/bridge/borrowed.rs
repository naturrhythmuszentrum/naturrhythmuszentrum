// SPDX-License-Identifier: AGPL-3.0-only
//! Shared readers expire before the exclusive reservation/capture phase can begin.
use super::{LcOmegaCandidate, LcOmegaOutput, LcShapeError, prepare_on};
#[cfg(test)]
use crate::ExecutionAccount;
use crate::LoanError;
use crate::loan::{lc_origin::LcSchedule, registry::State};
use crate::{Heap, HostChannel, LcShapeAxes, LcShapeTile, TileReader};
use nrz_bindings::{Budget, Registry, readings::algebra::enclosure::lc_root::Certificate};
use nrz_scalar::Scalar;

/// Authenticated original pins remain borrowed, without borrowing the heap or cloning an edge.
#[repr(C)]
pub(in crate::loan::lc_shape) struct LcBorrowedCapturePlan<'t, 'i, P: Scalar> {
    /// The same pins authenticated during the shared heap reborrow.
    pub(super) tiles: &'t [&'t LcShapeTile<'i, P>],
    /// Local schedule stamps and the arithmetic expectation checked against the sealed request.
    pub(super) schedule: LcSchedule,
}

/// Read-only portion of the consuming boundary, before exclusive reservation and capture.
///
/// The eventual consuming caller retains `&mut Heap` across this shared reborrow and
/// the following reservation/capture transaction. Nothing may publish or reclaim between them.
pub(in crate::loan::lc_shape) fn prepare_borrowed_on<
    't,
    'i,
    P: Scalar + 'static,
    const N: usize,
>(
    output: &LcOmegaOutput<'i, P, N>,
    heap: &Heap,
    tiles: &'t [&'t LcShapeTile<'i, P>],
    registry: &Registry,
    certificate: Option<&Certificate>,
    account: u64,
    budget: &mut Budget,
) -> Result<(LcOmegaCandidate, LcBorrowedCapturePlan<'t, 'i, P>), LcShapeError> {
    let first = output
        .inventory
        .input(0)
        .ok_or(LoanError::UnsupportedContract)?;
    if account != first.account {
        return Err(LoanError::ForeignAccount.into());
    }
    let external = certificate.map_or(0, Certificate::retained_bytes);
    budget.with_retained_payload(external, crate::loan::AT, |budget| {
        cheap::<P, N>(tiles, budget)
    })?;
    let candidate = prepare_on(output, registry, certificate, account, budget)?;
    budget.with_retained_payload(external, crate::loan::AT, |budget| {
        Ok::<_, LoanError>(candidate.authenticate(output, registry, budget))
    })??;
    let identity = account;
    // The current reader set, its copied axes and the per-row borrowed source,
    // reassociated projection and current companion coexist during row agreement.
    let scratch = size_of::<crate::LcInputReaders<'_, P, N>>()
        + size_of::<LcShapeAxes>()
        + size_of::<crate::loan::application::lc_source::LcMappedSource<'_>>()
        + size_of::<crate::CellProduct>()
        + size_of::<crate::QualifiedMaskVersion>();
    let live = candidate
        .retained_bytes()?
        .checked_add(external)
        .and_then(|bytes| bytes.checked_add(scratch as u64))
        .ok_or(LoanError::ByteOverflow)?;
    let plan = budget
        .with_retained_payload(live, crate::loan::AT, |budget| {
            // Neither the returned plan nor the candidate can carry this shared heap borrow.
            authenticate(output, heap, tiles, identity, budget)
        })
        .map_err(LcShapeError::Loan)?;
    Ok((candidate, plan))
}

/// The held replay input is reserved before these allocation-free native checks.
fn cheap<P: Scalar + 'static, const N: usize>(
    tiles: &[&LcShapeTile<'_, P>],
    budget: &mut Budget,
) -> Result<(), LoanError> {
    crate::loan::charge(budget, 2)?;
    if tiles.len() != N {
        return Err(LoanError::UnsupportedContract);
    }
    let first = tiles.first().ok_or(LoanError::UnsupportedContract)?;
    let pin = first
        .pin()
        .record
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    let bundle = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    if bundle.identity.format != crate::storage::Format::of::<P>() {
        return Err(LoanError::LayoutMismatch);
    }
    Ok(())
}

/// Unit controls retain the actual account while borrowing its unchanged ledger.
#[cfg(test)]
pub(in crate::loan::lc_shape) fn prepare_borrowed<'t, 'i, P: Scalar + 'static, const N: usize>(
    output: &LcOmegaOutput<'i, P, N>,
    heap: &Heap,
    tiles: &'t [&'t LcShapeTile<'i, P>],
    registry: &Registry,
    certificate: Option<&Certificate>,
    account: &mut ExecutionAccount,
) -> Result<(LcOmegaCandidate, LcBorrowedCapturePlan<'t, 'i, P>), LcShapeError> {
    prepare_borrowed_on(
        output,
        heap,
        tiles,
        registry,
        certificate,
        account.identity,
        &mut account.ledger,
    )
}

/// All row agreement is prepaid; inherited current gates keep their own original charges.
fn authenticate<'t, 'i, P: Scalar + 'static, const N: usize>(
    output: &LcOmegaOutput<'i, P, N>,
    heap: &Heap,
    tiles: &'t [&'t LcShapeTile<'i, P>],
    account: u64,
    budget: &mut Budget,
) -> Result<LcBorrowedCapturePlan<'t, 'i, P>, LoanError> {
    crate::loan::charge(budget, N.checked_mul(9).ok_or(LoanError::CountOverflow)?)?;
    let readers = output
        .inventory
        .readers_prepaid(heap, tiles, account, budget)?;
    let axes = readers.axes();
    for (index, tile) in tiles.iter().enumerate() {
        let reader = readers.reader(index).ok_or(LoanError::TargetMismatch)?;
        row(heap, tile, reader, &axes, budget)?;
    }
    Ok(LcBorrowedCapturePlan {
        tiles,
        schedule: LcSchedule {
            positions: output.positions,
            specialization: output.key,
            revisions: [1; 4],
            format: crate::storage::Format::of::<P>(),
        },
    })
}

/// Fixed source/mapped occupations, projection, layout and three qualified companions.
fn row<P: Scalar + 'static>(
    heap: &Heap,
    tile: &LcShapeTile<'_, P>,
    reader: &TileReader<'_, P>,
    axes: &LcShapeAxes,
    budget: &mut Budget,
) -> Result<(), LoanError> {
    let pin = tile
        .pin()
        .record
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    let mapped = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    if !std::ptr::eq(mapped, reader.bundle) || mapped.identity != tile.receipt().ticket.identity {
        return Err(LoanError::TargetMismatch);
    }
    let source = mapped.origin.lc_source()?;
    let State::Published(owner) = &heap.loans.group(source.identity)?.state else {
        return Err(LoanError::StaleDependency);
    };
    let original = owner.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    heap.current_owner(original, budget)?;
    let host_origin = original.origin.evidence.is_none()
        && original.endpoints.is_none()
        && original.retained_law.is_none();
    if !host_origin {
        return Err(LoanError::UnsupportedContract);
    }
    let symbols = &tile.schema().symbols;
    for (actual, expected) in [
        (&original.declaration.symbols, &symbols[0]),
        (&source.declaration.symbols, &symbols[0]),
        (&mapped.declaration.symbols, &symbols[1]),
    ] {
        if actual.value() != expected.value()
            || actual.bound() != expected.bound()
            || actual.error() != expected.error()
        {
            return Err(LoanError::TargetMismatch);
        }
    }
    let projection = axes
        .combined
        .reassociate_outer(axes.instance_samples, axes.sample_branches)?;
    if source.projection != projection
        || original.declaration.axis != axes.instances
        || mapped.declaration.axis != axes.combined.axis()
        || original.identity.layout != source.identity.layout
    {
        return Err(LoanError::LayoutMismatch);
    }
    for channel in HostChannel::ALL {
        let index = channel.index();
        let actual = mapped.qualified[index].ok_or(LoanError::TargetMismatch)?;
        if Some(actual) != tile.receipt().filled(channel)
            || actual.identity != mapped.identity
            || actual.channel != channel
            || original.qualified[index] != source.attachments[index]
            || source.attachments[index].is_none()
        {
            return Err(LoanError::TargetMismatch);
        }
    }
    Ok(())
}

/// A private pin/reader substitution cannot pass merely because both are valid native objects.
#[cfg(test)]
pub(super) fn defensive_pin_reader<P: Scalar + 'static>(
    heap: &Heap,
    tiles: &[&LcShapeTile<'_, P>],
    account: &mut ExecutionAccount,
) -> Result<(), LoanError> {
    let tile = tiles.first().ok_or(LoanError::UnsupportedContract)?;
    let other = tiles.get(1).ok_or(LoanError::UnsupportedContract)?;
    let reader = other.current_reader_on(heap, account.identity, &mut account.ledger)?;
    assert!(matches!(
        row(heap, tile, &reader, &tile.axes(), &mut account.ledger),
        Err(LoanError::TargetMismatch)
    ));
    Ok(())
}
