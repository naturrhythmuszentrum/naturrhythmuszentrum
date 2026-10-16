// SPDX-License-Identifier: AGPL-3.0-only
//! Operation-specific product derivative over all entries of both ordered operands.

use super::{
    StatisticsError,
    affine::Affine,
    matrix::{Matrix, singleton},
};
use crate::{CheckedOperation, OperationError};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// `J = [B^T tensor I, I tensor A]` in logical column-vector order.
pub(crate) fn map<P: Scalar + 'static, Q: Scalar + 'static>(
    operation: &CheckedOperation,
    points: &[Matrix<P>],
    context: &ArithmeticContext,
) -> Result<Affine<Q>, OperationError> {
    let left = points.first().ok_or(StatisticsError::Contract)?;
    let right = points.get(1).ok_or(StatisticsError::Contract)?;
    let shape = operation
        .inputs()
        .first()
        .ok_or(StatisticsError::Contract)?
        .shape()
        .extent();
    let output = operation
        .outputs()
        .first()
        .ok_or(StatisticsError::Contract)?
        .shape()
        .extent();
    let columns = left
        .centers
        .len()
        .checked_add(right.centers.len())
        .ok_or(StatisticsError::ResourceLimit)?;
    let mut result = Affine {
        jacobian: Matrix::zeros(output.entries(), columns)?,
        offsets: Matrix::zeros(output.entries(), 1)?,
    };
    for column in 0..output.columns() {
        for row in 0..output.rows() {
            let output_entry = output.index(row, column)?;
            let mut truth = Enclosure::singleton(Finite::ZERO);
            let mut center = Q::ZERO;
            for index in 0..shape.columns() {
                let left_entry = shape.index(row, index)?;
                let right_entry = index + shape.columns() * column;
                let first: Q = crate::conversion::same_format(
                    *left
                        .centers
                        .get(left_entry)
                        .ok_or(StatisticsError::Contract)?,
                )?;
                let second: Q = crate::conversion::same_format(
                    *right
                        .centers
                        .get(right_entry)
                        .ok_or(StatisticsError::Contract)?,
                )?;
                result
                    .jacobian
                    .set(output_entry, left_entry, second, singleton(second)?)?;
                result.jacobian.set(
                    output_entry,
                    left.centers.len() + right_entry,
                    first,
                    singleton(first)?,
                )?;
                truth = truth.add(singleton(first)?.mul(singleton(second)?, context)?, context)?;
                center = center + first * second;
            }
            result
                .offsets
                .set(output_entry, 0, -center, truth.negated())?;
        }
    }
    Ok(result)
}
