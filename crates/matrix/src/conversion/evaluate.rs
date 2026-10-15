// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical recipes fill private candidates; no heap mutation occurs here.

use super::{ConversionContract, ConversionError, ConversionEvidence};
use crate::binding::buffer;
use crate::coordinate_numeric::LoweredMap;
use crate::publication::Payload;
use crate::{NormalizedView, OperationError, OutputReservation, RecipeEvidence};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Evaluate one checked map with a per-execution exact-identity coefficient cache.
pub(crate) fn evaluate<P: Scalar + 'static, Q: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    reservation: OutputReservation,
    contract: &ConversionContract,
    cache: &mut Vec<LoweredMap<Q>>,
    context: &ArithmeticContext,
    slot: usize,
) -> Result<(Payload<Q>, RecipeEvidence), OperationError> {
    let position = if let Some(position) = cache
        .iter()
        .position(|lowered| lowered.exact == contract.map)
    {
        position
    } else {
        let lowered = contract
            .cache
            .try_borrow_mut()
            .map_err(|_| ConversionError::Contract)?
            .get(&contract.map, contract.literals, context)?;
        cache
            .try_reserve(1)
            .map_err(|_| ConversionError::ResourceLimit)?;
        cache.push(lowered);
        cache.len() - 1
    };
    let map = cache.get(position).ok_or(ConversionError::Contract)?;
    let payload = region(view, reservation, map, context)?;
    let evidence =
        ConversionEvidence::new(slot, payload.extent.entries(), reservation.coverage(), map);
    Ok((payload, RecipeEvidence::Conversion(Box::new(evidence))))
}

/// Form the center, complete bound and new evaluation region in logical column order.
fn region<P: Scalar + 'static, Q: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    reservation: OutputReservation,
    map: &LoweredMap<Q>,
    context: &ArithmeticContext,
) -> Result<Payload<Q>, OperationError> {
    let extent = view.extent();
    if extent != reservation.extent() || !reservation.bounded || !reservation.evaluation {
        return Err(ConversionError::Contract.into());
    }
    let mut centers = buffer(extent.entries())?;
    let mut totals = buffer(extent.entries())?;
    let mut errors = buffer(extent.entries())?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let converted = map.evaluate(
                super::same_format(view.get(row, column)?)?,
                super::same_format(view.bound(row, column)?.unwrap_or(P::ZERO))?,
                context,
            )?;
            centers.push(converted.center);
            totals.push(converted.total);
            errors.push(converted.evaluation);
        }
    }
    Ok(Payload {
        extent,
        target: reservation.stamp(),
        centers,
        bounds: Some(totals),
        evaluation: Some(errors),
    })
}
