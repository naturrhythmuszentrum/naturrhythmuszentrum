// SPDX-License-Identifier: AGPL-3.0-only
//! Paired measure maps scale both covariance axes while offsets act only on declared means.

use super::{StatisticsError, affine::Affine};
use crate::{CheckedOperation, ConversionContract, OperationError};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Reuse N's original coefficient cache and exact map identity in the statistical derivative.
pub(crate) fn map<P: Scalar + 'static>(
    operation: &CheckedOperation,
    contracts: &[ConversionContract],
    slot: usize,
    output: &mut Affine<P>,
    context: &ArithmeticContext,
) -> Result<(), OperationError> {
    let contract = contracts.get(slot).ok_or(StatisticsError::Contract)?;
    let lowered = contract
        .cache
        .try_borrow_mut()
        .map_err(|_| StatisticsError::Contract)?
        .get::<P>(&contract.map, contract.literals, context)?;
    let start = operation
        .inputs()
        .iter()
        .take(slot)
        .try_fold(0_usize, |sum, input| {
            sum.checked_add(input.shape().extent().entries())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
    for row in 0..output.jacobian.extent.rows() {
        output.jacobian.set(
            row,
            start + row,
            lowered.scale.center().get(),
            lowered.scale.enclosure(),
        )?;
        output.offsets.set(
            row,
            0,
            lowered.shift.center().get(),
            lowered.shift.enclosure(),
        )?;
    }
    Ok(())
}
