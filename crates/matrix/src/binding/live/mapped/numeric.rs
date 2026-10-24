// SPDX-License-Identifier: AGPL-3.0-only
//! Enclose the selected affine value and combine already mapped radius channels once.

use super::{CompanionEvaluation, Error, Heap, HeapError, ReadingSelection};
use crate::binding::live::parameters::numeric::{coefficients, contraction};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionReadingKind,
        algebra::{Location, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// One caller ledger accounts for selection lowering, value arithmetic and radius combination.
pub(super) const AT: Location = Location::new("mapped reading selection");

/// Complete value channels, private until all companion evaluation succeeds.
#[derive(Debug)]
pub(super) struct Payload<P: Scalar> {
    /// Rounded logical values after the original affine map.
    pub(super) centers: Vec<P>,
    /// Complete declared family plus value evaluation discrepancy.
    pub(super) totals: Vec<P>,
    /// Original numerical-only radius plus value evaluation discrepancy.
    pub(super) errors: Vec<P>,
}

/// Decode every coefficient and offset before borrowing selected numerical coordinates.
pub(super) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    selection: &ReadingSelection<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Payload<P>, Error> {
    let map = selection.source().reading().map();
    let decoded = coefficients::<P>(map.coefficients(), selection.literals(), context, budget)?;
    let offsets = coefficients::<P>(map.offsets(), selection.literals(), context, budget)?;
    let values = heap.reading_entries(selection.source())?;
    let count = usize::try_from(map.inputs()).map_err(|_| HeapError::ExtentOverflow)?;
    if values.len() != count {
        return Err(Error::Identity);
    }
    let mut zeros = crate::binding::buffer(count)?;
    zeros.resize(count, P::ZERO);
    let mut payload = Payload {
        centers: crate::binding::buffer(offsets.len())?,
        totals: crate::binding::buffer(offsets.len())?,
        errors: crate::binding::buffer(offsets.len())?,
    };
    for (row, offset) in offsets.iter().enumerate() {
        let start = row.checked_mul(count).ok_or(HeapError::ExtentOverflow)?;
        let end = start.checked_add(count).ok_or(HeapError::ExtentOverflow)?;
        let (center, error) = contraction(
            decoded.get(start..end).ok_or(Error::Identity)?,
            &values,
            &zeros,
            context,
            budget,
        )?;
        budget.work(4, AT)?;
        let enclosure = Enclosure::singleton(Finite::new(center)?)
            .add(Enclosure::new(-error, error)?, context)?
            .add(offset.enclosure(), context)?;
        let center = Finite::new(center + offset.center().get())?;
        let error = enclosure.displacement_from(center, context)?.get();
        payload.centers.push(center.get());
        payload.totals.push(error);
        payload.errors.push(error);
    }
    Ok(payload)
}

/// Parameter errors enclose the mapped radius itself; the value-map discrepancy is added once.
pub(super) fn radii<P: Scalar + 'static>(
    heap: &Heap,
    companions: &[CompanionEvaluation<P>],
    payload: &mut Payload<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(), Error> {
    for companion in companions {
        let CompanionReadingKind::Radius(kind) = companion.exact().target().kind() else {
            continue;
        };
        let values = companion.values(heap, context)?;
        let errors = companion.errors(heap, context)?;
        if values.len() != payload.centers.len() {
            return Err(Error::Identity);
        }
        let channel = match kind {
            CompanionKind::DeterministicBound => &mut payload.totals,
            CompanionKind::NumericalError => &mut payload.errors,
        };
        for ((radius, value), error) in channel.iter_mut().zip(values).zip(errors) {
            budget.work(4, AT)?;
            let upper = Enclosure::singleton(Finite::new(*value)?)
                .add(Enclosure::new(-*error, *error)?, context)?
                .upper()
                .get();
            if upper < P::ZERO {
                return Err(Error::Identity);
            }
            *radius = Enclosure::singleton(Finite::new(*radius)?)
                .add(Enclosure::singleton(Finite::new(upper)?), context)?
                .upper()
                .get();
        }
    }
    for (total, error) in payload.totals.iter_mut().zip(&payload.errors) {
        budget.work(1, AT)?;
        if *total < *error {
            *total = *error;
        }
    }
    Ok(())
}

/// Already stored output parameters must agree with the explicit original-source enclosure.
pub(super) fn declared<P: Scalar + 'static>(
    heap: &Heap,
    selection: &ReadingSelection<P>,
    companions: &[CompanionEvaluation<P>],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(), Error> {
    for (index, required) in selection.source().companions().iter().enumerate() {
        if !required
            .targets()
            .iter()
            .any(|target| target.reading() == selection.source().reading())
        {
            continue;
        }
        let evaluated = companions
            .iter()
            .find(|actual| actual.exact().target().node() == required.companion().node())
            .ok_or(Error::Identity)?;
        let bytes = add(0, required.entries().len(), std::mem::size_of::<P>())?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            u64::try_from(bytes).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        let stored = heap.reading_companion_entries(selection.source(), index)?;
        let values = evaluated.values(heap, context)?;
        let errors = evaluated.errors(heap, context)?;
        if stored.len() != values.len() {
            return Err(Error::Identity);
        }
        for ((stored, value), error) in stored.iter().zip(values).zip(errors) {
            budget.work(4, AT)?;
            let enclosure = Enclosure::singleton(Finite::new(*value)?)
                .add(Enclosure::new(-*error, *error)?, context)?;
            if *stored < enclosure.lower().get() || *stored > enclosure.upper().get() {
                return Err(Error::Identity);
            }
        }
    }
    Ok(())
}

/// Count all result payloads and owned headers before allocating any private output.
pub(super) fn owned<P: Scalar>(selection: &ReadingSelection<P>) -> Result<u64, Error> {
    let outputs = usize::try_from(selection.source().reading().map().outputs())
        .map_err(|_| HeapError::ExtentOverflow)?;
    let mut bytes = std::mem::size_of::<super::Snapshot<P>>() + 2 * std::mem::size_of::<usize>();
    bytes = add(bytes, outputs, 3 * std::mem::size_of::<P>())?;
    bytes = add(
        bytes,
        selection.companions().len(),
        std::mem::size_of::<CompanionEvaluation<P>>(),
    )?;
    for companion in selection.companions() {
        let count = usize::try_from(companion.exact().target().map().outputs())
            .map_err(|_| HeapError::ExtentOverflow)?;
        bytes = add(bytes, count, 2 * std::mem::size_of::<P>())?;
        bytes = add(
            bytes,
            1,
            2 * std::mem::size_of::<Vec<P>>() + 2 * std::mem::size_of::<usize>(),
        )?;
    }
    u64::try_from(bytes).map_err(|_| HeapError::ExtentOverflow.into())
}

/// Simultaneous decoding, source arrays and outputs fit one staged payload ceiling.
pub(super) fn reserve<P: Scalar>(
    selection: &ReadingSelection<P>,
    owned: u64,
    budget: &Budget,
) -> Result<(), Error> {
    let map = selection.source().reading().map();
    let count = map
        .coefficients()
        .len()
        .checked_add(map.offsets().len())
        .ok_or(HeapError::ExtentOverflow)?;
    let inputs = usize::try_from(map.inputs()).map_err(|_| HeapError::ExtentOverflow)?;
    let bytes = add(
        0,
        count,
        std::mem::size_of::<nrz_scalar::DecodedLiteral<P>>(),
    )?;
    let bytes = add(bytes, inputs, 2 * std::mem::size_of::<P>())?;
    let bytes = u64::try_from(bytes)
        .map_err(|_| HeapError::ExtentOverflow)?
        .checked_add(owned)
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(())
}

/// Checked byte arithmetic cannot admit a smaller allocation after overflow.
fn add(bytes: usize, count: usize, width: usize) -> Result<usize, HeapError> {
    count
        .checked_mul(width)
        .and_then(|value| bytes.checked_add(value))
        .ok_or(HeapError::ExtentOverflow)
}
