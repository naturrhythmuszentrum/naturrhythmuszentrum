// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical parameters retain actual result, operand and original source symbols.

use super::{MomentPayload, StatisticsError};
use crate::binding::parameters;
use crate::{Bound, Layout, OperationError};
use nrz_bindings::{ParameterSymbol, ParameterTarget};
use nrz_scalar::Scalar;

/// Predeclare covariance, composed sensitivity and local derivative as distinct symbols.
pub(super) fn moment<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
    payload: &MomentPayload<Q>,
    slot: usize,
) -> Result<[ParameterSymbol; 3], OperationError> {
    let output = bound
        .operation
        .outputs()
        .get(slot)
        .ok_or(StatisticsError::Contract)?;
    let rows = output_targets(bound, slot)?;
    let quantity = output.descriptor();
    let covariance = super::publish::covariance_descriptors(
        quantity,
        quantity,
        payload.covariance.extent.entries(),
    )?;
    let sensitivity = super::publish::source_descriptors(quantity, &payload.lineage, rows.len())?;
    let jacobian = super::publish::derivative_descriptors(quantity, &bound.operation, rows.len())?;
    let sources = payload
        .lineage
        .columns
        .iter()
        .map(|&index| {
            let symbol = payload.lineage.model.source(index)?.1;
            ParameterTarget::new(symbol.clone(), 0)
                .map_err(crate::HeapError::from)
                .map_err(OperationError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut inputs = Vec::new();
    for input in &bound.dependencies {
        inputs.extend(parameters::targets(input.symbols.value(), input.layout())?);
    }
    Ok([
        parameters::paired(payload.covariance.extent, &covariance, &rows, &rows)?,
        parameters::paired(payload.sensitivity.extent, &sensitivity, &rows, &sources)?,
        parameters::paired(payload.jacobian.extent, &jacobian, &rows, &inputs)?,
    ])
}

/// Exact result coordinate identities exist before result storage is published.
pub(super) fn output_targets<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
    slot: usize,
) -> Result<Vec<ParameterTarget>, OperationError> {
    let symbol = bound
        .reservations
        .symbols
        .get(slot)
        .ok_or(StatisticsError::Contract)?
        .value();
    let output = bound
        .operation
        .outputs()
        .get(slot)
        .ok_or(StatisticsError::Contract)?;
    Ok(parameters::targets(
        symbol,
        &Layout::contiguous(output.shape().extent())?,
    )?)
}
