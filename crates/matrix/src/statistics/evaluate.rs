// SPDX-License-Identifier: AGPL-3.0-only
//! Compose source sensitivities before contracting the complete joint covariance.

use super::{
    MomentPayload, Plan, StatisticsError,
    contraction::{Work, multiply},
    matrix::Matrix,
};
use crate::solve::scratch::{Attempt, Stage};
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
        let context = ArithmeticContext::new()?;
        let mut work = Work::new(self.work);
        let inputs = self.stacked::<Q>(&context)?;
        let covariance = lineage
            .model
            .covariance(&lineage.columns, &context, self.limits)?;
        let means = if lineage.assurance == super::MomentAssurance::AffineMoments {
            self.means::<Q>(&context)?
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
    fn stacked<Q: Scalar + 'static>(
        &self,
        context: &ArithmeticContext,
    ) -> Result<Matrix<Q>, OperationError> {
        let lineage = self.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        let rows = self.inputs.iter().try_fold(0_usize, |sum, input| {
            sum.checked_add(input.sensitivity.extent.rows())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
        let mut result = Matrix::zeros(rows, lineage.columns.len())?;
        self.stack_into(&mut result, context)?;
        Ok(result)
    }
    /// Fill planned source-sensitivity channels without changing the original source order.
    fn stack_into<Q: Scalar + 'static>(
        &self,
        result: &mut Matrix<Q>,
        context: &ArithmeticContext,
    ) -> Result<(), OperationError> {
        let lineage = self.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        let mut start = 0;
        for input in &self.inputs {
            for (original, source) in input.lineage.columns.iter().enumerate() {
                let column = lineage
                    .columns
                    .binary_search(source)
                    .map_err(|_| StatisticsError::Contract)?;
                for row in 0..input.sensitivity.extent.rows() {
                    let index = input.sensitivity.extent.index(row, original)?;
                    let (center, truth) = super::precision::entry(
                        *input
                            .sensitivity
                            .centers
                            .get(index)
                            .ok_or(StatisticsError::Contract)?,
                        input.sensitivity.entry(row, original)?,
                        context,
                    )?;
                    result.set(start + row, column, center, truth)?;
                }
            }
            start += input.sensitivity.extent.rows();
        }
        Ok(())
    }
    /// A nominal operating center is never silently promoted to an expectation.
    fn means<Q: Scalar + 'static>(
        &self,
        context: &ArithmeticContext,
    ) -> Result<Option<Matrix<Q>>, OperationError> {
        if self.inputs.iter().any(|input| input.means.is_none()) {
            return Ok(None);
        }
        let rows = self.inputs.iter().try_fold(0_usize, |sum, input| {
            sum.checked_add(input.sensitivity.extent.rows())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
        let mut result = Matrix::zeros(rows, 1)?;
        self.means_into(&mut result, context)?;
        Ok(Some(result))
    }
    /// Fill planned explicit means; a missing mean is still a typed refusal.
    fn means_into<Q: Scalar + 'static>(
        &self,
        result: &mut Matrix<Q>,
        context: &ArithmeticContext,
    ) -> Result<(), OperationError> {
        let mut start = 0;
        for input in &self.inputs {
            let means = input.means.as_ref().ok_or(StatisticsError::Contract)?;
            for row in 0..means.extent.rows() {
                let (center, truth) = super::precision::entry(
                    *means.centers.get(row).ok_or(StatisticsError::Contract)?,
                    means.entry(row, 0)?,
                    context,
                )?;
                result.set(start + row, 0, center, truth)?;
            }
            start += means.extent.rows();
        }
        Ok(())
    }
}

impl<P: Scalar + 'static> Plan<P> {
    /// Contract a certified solve differential using the same complete joint source relation.
    pub(crate) fn solve_moments<Q: Scalar + 'static>(
        &self,
        affine: super::Affine<Q>,
        context: &ArithmeticContext,
        run: &crate::solve::scratch::Run,
    ) -> Result<Vec<MomentPayload<Q>>, OperationError> {
        let lineage = self.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        let mut work = Work::new(self.work);
        let mut inputs = run.matrix(
            Stage::StatisticalInputs,
            Stage::StatisticalInputsTruth,
            None,
        )?;
        self.stack_into(&mut inputs, context)?;
        let mut covariance =
            run.matrix(Stage::StatisticalModel, Stage::StatisticalModelTruth, None)?;
        lineage
            .model
            .covariance_into(&lineage.columns, context, self.limits, &mut covariance)?;
        let sensitivity = super::contraction::multiply_planned(
            &affine.jacobian,
            &inputs,
            context,
            &mut work,
            Attempt { run, index: 0 },
        )?;
        drop(inputs);
        let first = super::contraction::multiply_planned(
            &sensitivity,
            &covariance,
            context,
            &mut work,
            Attempt { run, index: 1 },
        )?;
        drop(covariance);
        let mut transposed = run.matrix(
            Stage::StatisticalTranspose,
            Stage::StatisticalTransposeTruth,
            None,
        )?;
        sensitivity.transpose_into(&mut transposed)?;
        let output = super::contraction::multiply_planned(
            &first,
            &transposed,
            context,
            &mut work,
            Attempt { run, index: 2 },
        )?;
        drop(first);
        drop(transposed);
        let means = self.planned_means(&affine, context, &mut work, run)?;
        Ok(vec![MomentPayload {
            lineage: lineage.clone(),
            sensitivity,
            covariance: output,
            jacobian: affine.jacobian,
            derivative: self.derivative.clone(),
            means,
        }])
    }
    /// Explicit affine means retain their own planned source parameters and numerical errors.
    fn planned_means<Q: Scalar + 'static>(
        &self,
        affine: &super::Affine<Q>,
        context: &ArithmeticContext,
        work: &mut Work,
        run: &crate::solve::scratch::Run,
    ) -> Result<Option<Matrix<Q>>, OperationError> {
        let lineage = self.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        let means = if lineage.assurance == super::MomentAssurance::AffineMoments {
            if self.inputs.iter().all(|input| input.means.is_some()) {
                let mut means =
                    run.matrix(Stage::StatisticalMeans, Stage::StatisticalMeansTruth, None)?;
                self.means_into(&mut means, context)?;
                Some(means)
            } else {
                None
            }
        } else {
            None
        };
        means
            .as_ref()
            .map(|means| {
                let mut output = super::contraction::multiply_planned(
                    &affine.jacobian,
                    means,
                    context,
                    work,
                    Attempt { run, index: 3 },
                )?;
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
                            .add(affine.offsets.entry(row, 0)?, context)?,
                    )?;
                }
                Ok::<_, OperationError>(output)
            })
            .transpose()
    }
}
