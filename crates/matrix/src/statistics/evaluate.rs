// SPDX-License-Identifier: AGPL-3.0-only
//! Compose source sensitivities before contracting the complete joint covariance.

use super::{
    MomentPayload, Plan, StatisticsError,
    contraction::{Work, multiply},
    matrix::Matrix,
};
use crate::{CheckedOperation, OperationError};
use nrz_scalar::{ArithmeticContext, Scalar};

impl<P: Scalar + 'static> Plan<P> {
    /// Evaluate complete moments only after the deterministic candidate succeeds.
    pub(crate) fn evaluate<Q: Scalar + 'static>(
        &self,
        operation: &CheckedOperation,
    ) -> Result<Vec<MomentPayload<Q>>, OperationError> {
        let Some(lineage) = &self.lineage else {
            return Ok(Vec::new());
        };
        if P::PRECISION != Q::PRECISION {
            return Err(StatisticsError::Unsupported.into());
        }
        let context = ArithmeticContext::new()?;
        let mut work = Work::new(self.work);
        let inputs = self.stacked::<Q>()?;
        let covariance = lineage
            .model
            .covariance(&lineage.columns, &context, self.limits)?;
        let means = if lineage.assurance == super::MomentAssurance::AffineMoments {
            self.means::<Q>()?
        } else {
            None
        };
        let mut result = crate::binding::buffer(operation.outputs().len())?;
        for slot in 0..operation.outputs().len() {
            let affine = super::nonlinear::map(
                operation,
                slot,
                &self.points,
                self.derivative.as_deref(),
                &context,
            )?;
            let sensitivity = multiply(&affine.jacobian, &inputs, &context, &mut work)?;
            let first = multiply(&sensitivity, &covariance, &context, &mut work)?;
            let output = multiply(&first, &sensitivity.transpose()?, &context, &mut work)?;
            let mean = means
                .as_ref()
                .map(|means| {
                    let mut output = multiply(&affine.jacobian, means, &context, &mut work)?;
                    for row in 0..output.extent.rows() {
                        let center = *output.centers.get(row).ok_or(StatisticsError::Contract)?
                            + *affine
                                .offsets
                                .centers
                                .get(row)
                                .ok_or(StatisticsError::Contract)?;
                        output.set(
                            row,
                            0,
                            center,
                            output
                                .entry(row, 0)?
                                .add(affine.offsets.entry(row, 0)?, &context)?,
                        )?;
                    }
                    Ok::<_, OperationError>(output)
                })
                .transpose()?;
            result.push(MomentPayload {
                lineage: lineage.clone(),
                sensitivity,
                covariance: output,
                jacobian: affine.jacobian,
                derivative: self.derivative.clone(),
                means: mean,
            });
        }
        Ok(result)
    }
    /// Concatenate operand rows while remapping every column to its original joint source.
    fn stacked<Q: Scalar + 'static>(&self) -> Result<Matrix<Q>, OperationError> {
        let lineage = self.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        let rows = self.inputs.iter().try_fold(0_usize, |sum, input| {
            sum.checked_add(input.sensitivity.extent.rows())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
        let mut result = Matrix::zeros(rows, lineage.columns.len())?;
        let mut start = 0;
        for input in &self.inputs {
            for (original, source) in input.lineage.columns.iter().enumerate() {
                let column = lineage
                    .columns
                    .binary_search(source)
                    .map_err(|_| StatisticsError::Contract)?;
                for row in 0..input.sensitivity.extent.rows() {
                    let index = input.sensitivity.extent.index(row, original)?;
                    let center = crate::conversion::same_format(
                        *input
                            .sensitivity
                            .centers
                            .get(index)
                            .ok_or(StatisticsError::Contract)?,
                    )?;
                    let truth = input.sensitivity.entry(row, original)?;
                    result.set(
                        start + row,
                        column,
                        center,
                        nrz_scalar::Enclosure::new(
                            crate::conversion::same_format(truth.lower().get())?,
                            crate::conversion::same_format(truth.upper().get())?,
                        )?,
                    )?;
                }
            }
            start += input.sensitivity.extent.rows();
        }
        Ok(result)
    }
    /// A nominal operating center is never silently promoted to an expectation.
    fn means<Q: Scalar + 'static>(&self) -> Result<Option<Matrix<Q>>, OperationError> {
        if self.inputs.iter().any(|input| input.means.is_none()) {
            return Ok(None);
        }
        let rows = self.inputs.iter().try_fold(0_usize, |sum, input| {
            sum.checked_add(input.sensitivity.extent.rows())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
        let mut result = Matrix::zeros(rows, 1)?;
        let mut start = 0;
        for input in &self.inputs {
            let means = input.means.as_ref().ok_or(StatisticsError::Contract)?;
            for row in 0..means.extent.rows() {
                let truth = means.entry(row, 0)?;
                result.set(
                    start + row,
                    0,
                    crate::conversion::same_format(
                        *means.centers.get(row).ok_or(StatisticsError::Contract)?,
                    )?,
                    nrz_scalar::Enclosure::new(
                        crate::conversion::same_format(truth.lower().get())?,
                        crate::conversion::same_format(truth.upper().get())?,
                    )?,
                )?;
            }
            start += means.extent.rows();
        }
        Ok(Some(result))
    }
}
