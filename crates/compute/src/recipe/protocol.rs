// SPDX-License-Identifier: AGPL-3.0-only
//! Complete deterministic case protocol identity.

use super::{RecipeError, lanes, product};
use crate::{CaseRequest, Dependency, Operation};

/// Reset identity, including dry validation outside item timing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ResetPolicy {
    /// Reload fixed runtime seed state at each invocation.
    ReloadRuntimeSeeds,
    /// Clear mutable output outside each item timer.
    ZeroOutput,
    /// Restore matrix operands or outputs from their retained source.
    RestoreMatrixSource,
}

/// No cache flush or cold-cache claim is made by these recipes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CachePolicy {
    /// Reuse allocated storage without flushing the cache.
    NoFlushWarmReuse,
}

/// The parallel item timer includes coordinator channel dispatch and collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StartWaitPolicy {
    /// Execute on the calling thread without a channel protocol.
    DirectCall,
    /// Include generation-tagged worker dispatch and completion waits.
    BlockingChannel,
}

/// Matrix construction identity is distinct from its shape and observed performance.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Conditioning {
    /// Use centered finite seeded entries without a condition estimate.
    SeededCentered,
    /// Replace each diagonal by its ascending off-diagonal absolute sum plus margin.
    RowAbsoluteSumPlusPositiveMargin,
}

/// Owned numerical allocation strategy, separate from physical page residence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AllocationStrategy {
    /// Keep fixed arithmetic seed and coefficient arrays in one owned box.
    BoxedFixedState,
    /// Reserve each contiguous numerical vector fallibly.
    FallibleContiguousVectors,
    /// Keep numerical storage inside the matrix-owned raw handle.
    MatrixOwnedBuffers,
}

/// Fixed streaming summation order includes the scalar tail before final folding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReadOrder {
    /// Update eight vectors in order, add the scalar tail to matching partials, then fold.
    AccumulatorMajorLaneMinorTailThenFold,
}

/// Matrix seeded storage and diagonal construction order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MatrixConstruction {
    /// Generate centered entries in column-major order.
    ColumnMajorSeed,
    /// Generate column-major entries, then form row diagonals in ascending column order.
    ColumnMajorSeedAscendingRowSums,
}

/// A structure guarantee is not a computed condition-number estimate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConditionEstimate {
    /// No condition-number estimate was computed.
    UnknownNotEstimated,
}

/// Timed restoration occurs at a stated boundary within the repeated item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RestorationSchedule {
    /// No copy between prepared traversals is timed.
    None,
    /// Restore before each traversal, including the first.
    BeforeEveryTraversal,
    /// Restore only between consecutive traversals.
    BetweenTraversals,
}

/// Fixed case protocol and preparation identity, independent of observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseProtocol {
    /// Restoring inputs happens outside each item except the explicit repeated-factor scope.
    pub reset: ResetPolicy,
    /// Selected cache treatment.
    pub cache: CachePolicy,
    /// Coordinator/worker start and completion protocol.
    pub start_wait: StartWaitPolicy,
    /// Logical read partial sums; absent for other kernels.
    pub read_partials: Option<u64>,
    /// Scalar read tail elements after complete eight-vector chunks.
    pub read_tail: Option<u64>,
    /// Exact matrix construction recipe; margin is in the bound raw case inputs.
    pub conditioning: Option<Conditioning>,
    /// Source elements per timed restoration; schedule determines its repetition count.
    pub timed_restoration_elements: u64,
    /// Numerical allocation family.
    pub allocation: AllocationStrategy,
    /// Fixed read reduction ordering, absent for other families.
    pub read_order: Option<ReadOrder>,
    /// Explicit matrix generation order, absent for other families.
    pub matrix_construction: Option<MatrixConstruction>,
    /// Explicit lack of a computed condition-number estimate.
    pub condition_estimate: Option<ConditionEstimate>,
    /// Where each restoration occurs relative to repeated traversals.
    pub restoration_schedule: RestorationSchedule,
    /// Copied source bytes for one timed restoration.
    pub restoration_copied_bytes: u64,
    /// Logical read plus write traffic for one timed restoration.
    pub restoration_logical_bytes: u64,
}

impl CaseProtocol {
    /// Derive the fixed protocol from the explicit catalog case.
    pub(crate) fn from_case(case: &CaseRequest) -> Result<Self, RecipeError> {
        let matrix = case.dependency == Dependency::BackendKernel;
        let read = case.operation == Operation::Read;
        let partials = 8 * lanes(case.width);
        let conditioned = matches!(case.operation, Operation::Factor | Operation::Solve);
        let repeated = case.scope == crate::Scope::RestorationInclusiveRepeated;
        let solve = case.operation == Operation::Solve && case.scope == crate::Scope::Prepared;
        let elements = if repeated {
            let n = *case.shape.first().ok_or(RecipeError::Domain)?;
            product(&[n, n])?
        } else if solve {
            product(&case.shape)?
        } else {
            0
        };
        Ok(Self {
            allocation: if matrix {
                AllocationStrategy::MatrixOwnedBuffers
            } else if matches!(
                case.dependency,
                Dependency::Dependent | Dependency::Independent
            ) {
                AllocationStrategy::BoxedFixedState
            } else {
                AllocationStrategy::FallibleContiguousVectors
            },
            read_order: read.then_some(ReadOrder::AccumulatorMajorLaneMinorTailThenFold),
            matrix_construction: matrix.then_some(if conditioned {
                MatrixConstruction::ColumnMajorSeedAscendingRowSums
            } else {
                MatrixConstruction::ColumnMajorSeed
            }),
            condition_estimate: matrix.then_some(ConditionEstimate::UnknownNotEstimated),
            restoration_schedule: if repeated {
                RestorationSchedule::BeforeEveryTraversal
            } else if solve {
                RestorationSchedule::BetweenTraversals
            } else {
                RestorationSchedule::None
            },
            restoration_copied_bytes: elements
                .checked_mul(8)
                .filter(|n| *n <= crate::decimal::MAX_INTEGER)
                .ok_or(RecipeError::Overflow)?,
            restoration_logical_bytes: elements
                .checked_mul(16)
                .filter(|n| *n <= crate::decimal::MAX_INTEGER)
                .ok_or(RecipeError::Overflow)?,
            reset: if matrix {
                ResetPolicy::RestoreMatrixSource
            } else if matches!(
                case.dependency,
                Dependency::Dependent | Dependency::Independent
            ) {
                ResetPolicy::ReloadRuntimeSeeds
            } else {
                ResetPolicy::ZeroOutput
            },
            cache: CachePolicy::NoFlushWarmReuse,
            start_wait: if case.workers == 1 {
                StartWaitPolicy::DirectCall
            } else {
                StartWaitPolicy::BlockingChannel
            },
            read_partials: read.then_some(partials),
            read_tail: if read {
                Some(case.shape.first().ok_or(RecipeError::Domain)? % partials)
            } else {
                None
            },
            conditioning: matrix.then_some(if conditioned {
                Conditioning::RowAbsoluteSumPlusPositiveMargin
            } else {
                Conditioning::SeededCentered
            }),
            timed_restoration_elements: elements,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restoration_ledgers_distinguish_every_factor_from_between_solves() -> Result<(), RecipeError>
    {
        let mut case = crate::request::tests::request().cases.remove(0);
        case.operation = Operation::Factor;
        case.dependency = Dependency::BackendKernel;
        case.width = crate::Width::BackendSelectedBinary64;
        case.scope = crate::Scope::RestorationInclusiveRepeated;
        case.shape = vec![3];
        let factor = CaseProtocol::from_case(&case)?;
        assert_eq!(
            factor.restoration_schedule,
            RestorationSchedule::BeforeEveryTraversal
        );
        assert_eq!(factor.timed_restoration_elements, 9);
        assert_eq!(factor.restoration_copied_bytes, 72);
        assert_eq!(factor.restoration_logical_bytes, 144);
        case.operation = Operation::Solve;
        case.scope = crate::Scope::Prepared;
        case.shape = vec![2, 3];
        let solve = CaseProtocol::from_case(&case)?;
        assert_eq!(
            solve.restoration_schedule,
            RestorationSchedule::BetweenTraversals
        );
        assert_eq!(solve.timed_restoration_elements, 6);
        assert_eq!(solve.restoration_copied_bytes, 48);
        assert_eq!(solve.restoration_logical_bytes, 96);
        assert_eq!(
            solve.condition_estimate,
            Some(ConditionEstimate::UnknownNotEstimated)
        );
        case.scope = crate::Scope::PreparationInclusive;
        assert_eq!(
            CaseProtocol::from_case(&case)?.restoration_schedule,
            RestorationSchedule::None
        );
        Ok(())
    }
}
