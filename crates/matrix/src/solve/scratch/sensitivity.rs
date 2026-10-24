// SPDX-License-Identifier: AGPL-3.0-only
//! The original differential equation fixes every nominal, directional and Jacobian array.

use super::{Stage, plan::Specification, schedule::specification};
use crate::{Extent, OperationError};

impl Stage {
    /// Each certified nominal or differential solve has the same seven distinct numerical owners.
    const SENSITIVITY: [Self; 7] = [
        Self::SensitivityRightHandSide,
        Self::SensitivityRightHandSideTruth,
        Self::SensitivityPackedRightHandSide,
        Self::SensitivityBackendResult,
        Self::SensitivityCenters,
        Self::SensitivityResiduals,
        Self::SensitivityTruth,
    ];
    /// Closed sensitivity roles remain distinct from deterministic recovery attempts.
    pub(crate) const fn is_sensitivity(self) -> bool {
        matches!(
            self,
            Self::SensitivityRightHandSide
                | Self::SensitivityRightHandSideTruth
                | Self::SensitivityPackedRightHandSide
                | Self::SensitivityBackendResult
                | Self::SensitivityCenters
                | Self::SensitivityResiduals
                | Self::SensitivityTruth
                | Self::JacobianCenters
                | Self::JacobianTruth
                | Self::OffsetCenters
                | Self::OffsetTruth
                | Self::OutputJacobianCenters
                | Self::OutputJacobianTruth
                | Self::OutputOffsetCenters
                | Self::OutputOffsetTruth
        )
    }
    /// Candidate channels keep their live owners through publication; offsets are consumed earlier.
    pub(crate) const fn is_candidate(self) -> bool {
        matches!(
            self,
            Self::OutputCenters
                | Self::OutputTotals
                | Self::OutputErrors
                | Self::OutputJacobianCenters
                | Self::OutputJacobianTruth
        )
    }
    /// An enclosure is one logical entry containing two independently represented endpoints.
    pub(crate) const fn components(self) -> usize {
        if matches!(
            self,
            Self::SensitivityRightHandSideTruth
                | Self::SensitivityTruth
                | Self::JacobianTruth
                | Self::OffsetTruth
                | Self::OutputJacobianTruth
                | Self::OutputOffsetTruth
                | Self::StatisticalInputsTruth
                | Self::StatisticalModelTruth
                | Self::StatisticalTransposeTruth
                | Self::StatisticalMeansTruth
                | Self::ContractionTruth
        ) {
            2
        } else {
            1
        }
    }
}
/// Complete concatenated original operator/RHS order excludes a source-free identity RHS.
fn columns(contract: &super::super::SolveContract) -> Result<usize, OperationError> {
    contract.inputs.iter().try_fold(0_usize, |sum, input| {
        sum.checked_add(input.shape().extent().entries())
            .ok_or(OperationError::ResourceLimit)
    })
}
/// Count without speculative numerical or schedule allocation.
pub(super) fn count(
    contract: &super::super::SolveContract,
    present: bool,
) -> Result<usize, OperationError> {
    if !present {
        return Ok(0);
    }
    columns(contract)?
        .checked_add(1)
        .and_then(|n| n.checked_mul(7))
        .and_then(|n| n.checked_add(8))
        .ok_or(OperationError::ResourceLimit)
}
/// Every shape and directional coordinate is fixed before the first nominal RHS allocation.
pub(super) fn specifications(
    contract: &super::super::SolveContract,
    present: bool,
    result: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    if !present {
        return Ok(());
    }
    let columns = columns(contract)?;
    let extent = contract.solution.shape().extent();
    for direction in 0..=columns {
        for stage in Stage::SENSITIVITY {
            result.push(specification(stage, Some(direction), extent));
        }
    }
    let jacobian = Extent::new(extent.entries(), columns)?;
    let offsets = Extent::new(extent.entries(), 1)?;
    for stage in [
        Stage::JacobianCenters,
        Stage::JacobianTruth,
        Stage::OutputJacobianCenters,
        Stage::OutputJacobianTruth,
    ] {
        result.push(specification(stage, None, jacobian));
    }
    for stage in [
        Stage::OffsetCenters,
        Stage::OffsetTruth,
        Stage::OutputOffsetCenters,
        Stage::OutputOffsetTruth,
    ] {
        result.push(specification(stage, None, offsets));
    }
    Ok(())
}
