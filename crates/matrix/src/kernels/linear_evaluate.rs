// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed affine contraction encloses coefficient error both at the center and over its radius.

use super::arithmetic::{Entry, finish, operand, singleton};
use super::lowering::{Coefficient, lower};
use super::{KernelError, LinearMap};
use crate::binding::buffer;
use crate::{NormalizedView, OperationError};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Lower fixed definitions once per execution, then contract in increasing input-index order.
pub(crate) fn evaluate<P: Scalar + 'static>(
    map: &LinearMap,
    input: &NormalizedView<'_, P>,
    context: &ArithmeticContext,
) -> Result<Vec<Entry<P>>, OperationError> {
    let mut coefficients = buffer(map.coefficients.len())?;
    for (definition, meaning) in map.coefficients.iter().zip(map.operator.entries()) {
        let coefficient = lower(definition, map.literals, context)?;
        if meaning.is_zero()
            && (coefficient.enclosure.lower().get() != P::ZERO
                || coefficient.enclosure.upper().get() != P::ZERO)
        {
            return Err(KernelError::Contract.into());
        }
        coefficients.push(coefficient);
    }
    let extent = map.output.shape().extent();
    let mut result = buffer(extent.entries())?;
    for (index, offset) in map.offsets.iter().enumerate() {
        let shift = lower(offset, map.literals, context)?;
        result.push(contract(map, input, &coefficients, index, shift, context)?);
    }
    Ok(result)
}

/// Separate the exact-definition center enclosure from the propagated nonnegative radius.
fn contract<P: Scalar + 'static>(
    map: &LinearMap,
    input: &NormalizedView<'_, P>,
    coefficients: &[Coefficient<P>],
    index: usize,
    shift: Coefficient<P>,
    context: &ArithmeticContext,
) -> Result<Entry<P>, OperationError> {
    let extent = map.output.shape().extent();
    let row = index % extent.rows();
    let column = index / extent.rows();
    let mut center = shift.center;
    let mut nominal = shift.enclosure;
    let mut radius = Nonnegative::new(P::ZERO).map_err(KernelError::from)?;
    for contracted in 0..map.operator.extent().columns() {
        let coefficient = coefficients
            .get(map.operator.extent().index(row, contracted)?)
            .ok_or(KernelError::Contract)?;
        let (value, bound) = operand(input, input.extent().index(contracted, column)?)?;
        center = center + coefficient.center * value;
        nominal = nominal
            .add(
                coefficient
                    .enclosure
                    .mul(singleton(value)?, context)
                    .map_err(KernelError::from)?,
                context,
            )
            .map_err(KernelError::from)?;
        let magnitude = Nonnegative::new(coefficient.enclosure.abs().upper().get())
            .map_err(KernelError::from)?;
        radius = radius
            .add(
                magnitude.scale(bound, context).map_err(KernelError::from)?,
                context,
            )
            .map_err(KernelError::from)?;
    }
    Ok(finish(center, nominal, radius, context)?)
}
