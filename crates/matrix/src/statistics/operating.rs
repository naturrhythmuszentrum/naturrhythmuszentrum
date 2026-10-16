// SPDX-License-Identifier: AGPL-3.0-only
//! Retain the actual operating point and deterministic domain independently of covariance.

use super::{
    MomentAssurance, Plan, StatisticsError,
    matrix::{Matrix, enclosed},
};
use crate::{CheckedOperation, Handle, Heap, OperationError, Value, binding::NativeRecipe};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Copy the current frozen centers and complete deterministic boxes in operation order.
pub(crate) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    values: &[Handle<Value, P>],
    context: &ArithmeticContext,
) -> Result<Vec<Matrix<P>>, OperationError> {
    let mut result = crate::binding::buffer(values.len())?;
    for &handle in values {
        let value = heap.value(handle)?;
        let bounds = heap.selected_bound(handle)?;
        let extent = value.region().extent();
        let mut point = Matrix::zeros(extent.rows(), extent.columns())?;
        for column in 0..extent.columns() {
            for row in 0..extent.rows() {
                let center = value.region().get(row, column)?;
                let radius = bounds
                    .as_ref()
                    .map(|bound| bound.region().get(row, column))
                    .transpose()?
                    .unwrap_or(P::ZERO);
                point.set(row, column, center, enclosed(center, radius, context)?)?;
            }
        }
        result.push(point);
    }
    Ok(result)
}
impl<P: Scalar + 'static> Plan<P> {
    /// Affine operations retain inherited approximation; nonlinear local models never gain exact moments.
    pub(crate) fn classify(
        &mut self,
        operation: &CheckedOperation,
        context: &ArithmeticContext,
    ) -> Result<(), OperationError> {
        if matches!(operation.recipe, NativeRecipe::Solve(_)) {
            if !self.fixed(0, context)? {
                self.lineage
                    .as_mut()
                    .ok_or(StatisticsError::Contract)?
                    .assurance = MomentAssurance::LinearizedMoments;
            }
            return Ok(());
        }
        let NativeRecipe::Kernel(contract) = &operation.recipe else {
            return Ok(());
        };
        let nonlinear = match contract.kind() {
            crate::KernelKind::Elementwise(_) => true,
            crate::KernelKind::Product => !(self.fixed(0, context)? || self.fixed(1, context)?),
            _ => false,
        };
        if nonlinear {
            self.lineage
                .as_mut()
                .ok_or(StatisticsError::Contract)?
                .assurance = MomentAssurance::LinearizedMoments;
        }
        Ok(())
    }
    /// A modeled factor is mathematically fixed only with zero variation and an exact mean at c.
    pub(crate) fn fixed(
        &self,
        slot: usize,
        context: &ArithmeticContext,
    ) -> Result<bool, OperationError> {
        let input = self.inputs.get(slot).ok_or(StatisticsError::Contract)?;
        let point = self.points.get(slot).ok_or(StatisticsError::Contract)?;
        let Some(means) = &input.means else {
            return Ok(false);
        };
        if means
            .truth
            .iter()
            .zip(&point.centers)
            .any(|(mean, center)| mean.lower().get() != *center || mean.upper().get() != *center)
        {
            return Ok(false);
        }
        let covariance =
            input
                .lineage
                .model
                .covariance::<P>(&input.lineage.columns, context, self.limits)?;
        for column in 0..input.sensitivity.extent.columns() {
            let variance = covariance.entry(column, column)?;
            if variance.lower().get() == P::ZERO && variance.upper().get() == P::ZERO {
                continue;
            }
            for row in 0..input.sensitivity.extent.rows() {
                let coefficient = input.sensitivity.entry(row, column)?;
                if coefficient.lower().get() != P::ZERO || coefficient.upper().get() != P::ZERO {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
