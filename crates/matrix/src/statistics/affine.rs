// SPDX-License-Identifier: AGPL-3.0-only
//! Closed exact affine maps act on complete logical operand vectors and source rows.

use super::{
    StatisticsError,
    matrix::{Matrix, singleton},
};
use crate::kernels::contract::Recipe;
use crate::{CheckedOperation, OperationError, binding::NativeRecipe};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Local affine derivative and source-free offsets in output column order.
#[derive(Debug)]
pub(crate) struct Affine<P: Scalar> {
    /// Complete derivative over concatenated operands.
    pub(crate) jacobian: Matrix<P>,
    /// One exact affine offset per logical output coordinate.
    pub(crate) offsets: Matrix<P>,
}

/// Derive only a closed checked recipe; caller-provided numerical Jacobians are never accepted.
pub(crate) fn map<P: Scalar + 'static>(
    operation: &CheckedOperation,
    slot: usize,
    context: &ArithmeticContext,
) -> Result<Affine<P>, OperationError> {
    let rows = operation
        .outputs()
        .get(slot)
        .ok_or(StatisticsError::Contract)?
        .shape()
        .extent()
        .entries();
    let columns = operation.inputs().iter().try_fold(0_usize, |sum, region| {
        sum.checked_add(region.shape().extent().entries())
            .ok_or(StatisticsError::ResourceLimit)
    })?;
    let mut result = Affine {
        jacobian: Matrix::zeros(rows, columns)?,
        offsets: Matrix::zeros(rows, 1)?,
    };
    match &operation.recipe {
        NativeRecipe::Copy | NativeRecipe::Precision { .. } => {
            identity(operation, slot, &mut result.jacobian)?;
        }
        NativeRecipe::Kernel(contract) => match &contract.recipe {
            Recipe::Sum { subtract, right } => {
                for (row, &source) in right.indices().iter().enumerate() {
                    result.jacobian.set(row, row, P::ONE, singleton(P::ONE)?)?;
                    let sign = if *subtract { -P::ONE } else { P::ONE };
                    result.jacobian.set(
                        row,
                        rows.checked_add(source)
                            .ok_or(StatisticsError::ResourceLimit)?,
                        sign,
                        singleton(sign)?,
                    )?;
                }
            }
            Recipe::Transform(map) => {
                for (row, &source) in map.indices().iter().enumerate() {
                    result
                        .jacobian
                        .set(row, source, P::ONE, singleton(P::ONE)?)?;
                }
            }
            Recipe::Linear(map) => fixed(map, &mut result, context)?,
            Recipe::Product | Recipe::Elementwise(_) => {
                return Err(StatisticsError::Unsupported.into());
            }
        },
        NativeRecipe::Affine(contracts) => {
            super::conversion::map(operation, contracts, slot, &mut result, context)?;
        }
        NativeRecipe::Solve(_) => return Err(StatisticsError::Unsupported.into()),
    }
    Ok(result)
}

/// A copy selects only its own operand within the complete concatenation.
fn identity<P: Scalar>(
    operation: &CheckedOperation,
    slot: usize,
    output: &mut Matrix<P>,
) -> Result<(), OperationError> {
    let start = operation
        .inputs()
        .iter()
        .take(slot)
        .try_fold(0_usize, |sum, input| {
            sum.checked_add(input.shape().extent().entries())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
    for row in 0..output.extent.rows() {
        output.set(
            row,
            start
                .checked_add(row)
                .ok_or(StatisticsError::ResourceLimit)?,
            P::ONE,
            singleton(P::ONE)?,
        )?;
    }
    Ok(())
}

/// Matrix multiplication by a fixed operator repeats the same map across input columns.
fn fixed<P: Scalar + 'static>(
    map: &crate::LinearMap,
    output: &mut Affine<P>,
    context: &ArithmeticContext,
) -> Result<(), OperationError> {
    let rows = map.operator.extent().rows();
    let contracted = map.operator.extent().columns();
    for block in 0..map.input.shape().extent().columns() {
        for column in 0..contracted {
            for row in 0..rows {
                let coefficient = map
                    .coefficients
                    .get(map.operator.extent().index(row, column)?)
                    .ok_or(StatisticsError::Contract)?;
                let lowered = crate::kernels::lowering::lower(coefficient, map.literals, context)?;
                output.jacobian.set(
                    row + rows * block,
                    column + contracted * block,
                    lowered.center,
                    lowered.enclosure,
                )?;
            }
        }
    }
    for (row, offset) in map.offsets.iter().enumerate() {
        let lowered = crate::kernels::lowering::lower(offset, map.literals, context)?;
        output
            .offsets
            .set(row, 0, lowered.center, lowered.enclosure)?;
    }
    Ok(())
}
