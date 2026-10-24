// SPDX-License-Identifier: AGPL-3.0-only
//! The finite checked policy fixes every permitted allocation before backend entry.

use super::plan::{Specification, State};
pub(crate) use super::record::SolveAllocationStage as Stage;
use crate::{Extent, OperationError};
use std::cell::Cell;

impl Stage {
    /// Every stage of a single attempt, with inapplicable entries removed before allocation.
    const ATTEMPT: [Self; 13] = [
        Self::CorrectionRightHandSide,
        Self::PackedRightHandSide,
        Self::BackendResult,
        Self::Centers,
        Self::UpdatedCenters,
        Self::RetainedCenters,
        Self::Residuals,
        Self::OperatorRadii,
        Self::RightHandSideRadii,
        Self::RestoredCenters,
        Self::Totals,
        Self::Errors,
        Self::RestorationErrors,
    ];
    /// Final same-format copies belong to the candidate, independently of its attempt count.
    pub(crate) const OUTPUT: [Self; 3] =
        [Self::OutputCenters, Self::OutputTotals, Self::OutputErrors];
}

/// A policy permits a finite prefix; successful early termination leaves the remaining suffix unused.
pub(super) fn attempts(contract: &super::super::SolveContract) -> Result<usize, OperationError> {
    let count = contract.policy.recovery().map_or(1, |(policy, limits)| {
        u64::from(policy.max_refinements())
            .saturating_add(1)
            .min(u64::from(limits.max_attempts()))
    });
    usize::try_from(count).map_err(|_| OperationError::ResourceLimit)
}

/// The same selector is used for counting and constructing the plan, before any floating allocation.
const fn applicable(
    stage: Stage,
    iteration: usize,
    contract: &super::super::SolveContract,
) -> bool {
    match stage {
        Stage::CorrectionRightHandSide | Stage::UpdatedCenters => iteration != 0,
        Stage::RetainedCenters => contract.policy.recovery().is_some(),
        Stage::RestoredCenters | Stage::RestorationErrors => contract.restoration.is_some(),
        _ => true,
    }
}

/// Checked count permits metadata admission without allocating a speculative schedule.
pub(super) fn count(contract: &super::super::SolveContract) -> Result<usize, OperationError> {
    let attempts = attempts(contract)?;
    if attempts == 0 {
        return Err(OperationError::ResourceLimit);
    }
    let first = Stage::ATTEMPT
        .iter()
        .filter(|&&s| applicable(s, 0, contract))
        .count();
    let later = Stage::ATTEMPT
        .iter()
        .filter(|&&s| applicable(s, 1, contract))
        .count();
    later
        .checked_mul(attempts - 1)
        .and_then(|v| v.checked_add(first + 3))
        .ok_or(OperationError::ResourceLimit)
}

/// Every ordered coordinate follows the original checked operator or solution rectangle.
pub(super) fn specifications(
    contract: &super::super::SolveContract,
    result: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    let operator = contract
        .inputs
        .first()
        .ok_or(OperationError::InputCount)?
        .shape()
        .extent();
    let solution = contract.solution.shape().extent();
    for iteration in 0..attempts(contract)? {
        for stage in Stage::ATTEMPT {
            if applicable(stage, iteration, contract) {
                let extent = if stage == Stage::OperatorRadii {
                    operator
                } else {
                    solution
                };
                result.push(specification(stage, Some(iteration), extent));
            }
        }
    }
    for stage in Stage::OUTPUT {
        result.push(specification(stage, None, solution));
    }
    Ok(())
}

/// Metadata has a complete meaning even while its numerical stage remains unissued.
pub(super) const fn specification(
    stage: Stage,
    iteration: Option<usize>,
    extent: Extent,
) -> Specification {
    Specification {
        stage,
        iteration,
        extent,
        state: Cell::new(State::Planned),
        bytes: Cell::new(None),
    }
}
