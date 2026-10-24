// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-family structural zeros constrain statistical means and variations as well as centers.

use super::{MomentPayload, StatisticsError, capture::Input};
use crate::{CheckedOperation, CheckedShape, JointLimits, OperationError, StructureScope};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// A center-only zero cannot silently restrict a carried random coordinate.
pub(crate) fn inputs<P: Scalar + 'static>(
    operation: &CheckedOperation,
    inputs: &[Input<P>],
    context: &ArithmeticContext,
    limits: JointLimits,
    run: &super::inventory::Run,
) -> Result<(), OperationError> {
    for (slot, (contract, input)) in operation.inputs().iter().zip(inputs).enumerate() {
        let shape = contract.shape();
        if shape.structure_scope() != StructureScope::WholeFamily
            || !shape.entries().iter().any(crate::EntryContract::is_zero)
        {
            continue;
        }
        let mut covariance = run.matrix(super::inventory::Role::StructureCovariance, slot)?;
        input.lineage.model.covariance_into::<P>(
            &input.lineage.columns,
            context,
            limits,
            &mut covariance,
        )?;
        for (entry, meaning) in shape.entries().iter().enumerate() {
            if !meaning.is_zero() {
                continue;
            }
            let mean = input
                .means
                .as_ref()
                .ok_or(OperationError::StructureMismatch { slot, entry })?;
            if !zero(mean.entry(entry, 0)?) {
                return Err(OperationError::StructureMismatch { slot, entry });
            }
            for column in 0..input.sensitivity.extent.columns() {
                if !zero(input.sensitivity.entry(entry, column)?)
                    && !zero(covariance.entry(column, column)?)
                {
                    return Err(OperationError::StructureMismatch { slot, entry });
                }
            }
        }
    }
    Ok(())
}
/// Publish a whole-family zero only when the complete statistical payload proves it.
pub(crate) fn output<P: Scalar>(
    shape: &CheckedShape,
    payload: &MomentPayload<P>,
    slot: usize,
) -> Result<(), OperationError> {
    if shape.structure_scope() != StructureScope::WholeFamily {
        return Ok(());
    }
    for (entry, meaning) in shape.entries().iter().enumerate() {
        if !meaning.is_zero() {
            continue;
        }
        let means = payload
            .means
            .as_ref()
            .ok_or(OperationError::StructureMismatch { slot, entry })?;
        if !zero(means.entry(entry, 0)?) || !zero(payload.covariance.entry(entry, entry)?) {
            return Err(OperationError::StructureMismatch { slot, entry });
        }
    }
    if payload.covariance.extent.rows() != shape.extent().entries() {
        return Err(StatisticsError::Contract.into());
    }
    Ok(())
}
/// Exact endpoint zero is a proof; a rounded zero candidate is insufficient.
fn zero<P: Scalar>(value: Enclosure<P>) -> bool {
    value.lower().get() == P::ZERO && value.upper().get() == P::ZERO
}
