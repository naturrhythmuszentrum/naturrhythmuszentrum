// SPDX-License-Identifier: AGPL-3.0-only
//! Closed deterministic region kernels under the existing transaction owner.
//!
//! # Closed deterministic kernels and evidence
//!
//! `OperationDraft::add` and `subtract` derive a single checked output from two ordered homogeneous difference regions. Both input totals enter the outward radius even when the operands are the same handle or overlapping aliases. A distinct evaluation region encloses the discrepancy between the floating center and the exact represented-center operation. That error already belongs to the total and must not be added again. Different deterministic coverage scopes refuse with `KernelError::CoverageMismatch`; statistical companions follow the complete joint affine propagation documented in `src/statistics/`.
//!
//! `LinearMap::new` checks a fixed source-free operator, operand shape and affine offsets. `FixedCoefficient::represented` defines an exact binary64 mathematical constant; `descriptor` accepts a bindings-owned exact descriptor constant from the same frozen snapshot. Coefficients are enclosed in the execution format before use. The total includes `abs(L) r` with outward coefficient enclosures plus the complete floating and coefficient displacement at the represented center. Uncertain coefficient values belong in numerical operand regions, not fixed definitions. The output shape retains the derived coordinate axes, while offsets can fill entries that were structural zeros in the homogeneous product.
//!
//! `OperationDraft::transform_entries` checks selection, repetition and reshape against explicit new axes and an `EntryMap`. `dual_transpose` retains the reciprocal formal dual axes and permutes each total exactly. Both publish fresh values and totals with zero new evaluation error. Coherent formal dual references retain their primal reference definitions with reciprocal interpretation; general scaled formal-dual normalization still refuses. These operations preserve dependence through the original source selection and do not claim independent copies.
//!
//! Every kernel runs through `Binding`, normalized borrowed views, private candidates and atomic publication. Output reservations use the actual result extent. `RecipeEvidence::Kernel` exposes the closed `KernelKind`, precision, arithmetic revision and conservative admitted work count. Limits include numerical verification and complete output payloads. Only the private evaluator creates kernel evidence. Publication checks its recipe identity, all source dependencies, complete companion inventories and target versions. A multi-input result records every parent; a change to either operand invalidates downstream claims. `OperationRecord::target_map` refuses a multi-input output because it has no single source/destination pair; the operation record and provenance retain the complete relation.

pub(crate) mod arithmetic;
mod backend_product;
pub(crate) mod contract;
mod contributions;
mod evaluate;
pub(crate) mod linear;
mod linear_evaluate;
pub(crate) mod lowering;
mod nonlinear;
mod product;
pub(crate) mod reciprocal;

pub use crate::binding::kernel::linear::{FixedCoefficient, LinearMap};
pub(crate) use contract::KernelContract;
pub use contract::{KernelEvidence, KernelKind};
pub use contributions::ProductTerm;
pub(crate) use contributions::{
    ContributionReservation, finish as finish_contributions, views as contribution_views,
};
pub use nonlinear::{DomainReason, Elementwise};

/// A deterministic kernel refusal, without a partially published result.
#[derive(Clone, Debug, thiserror::Error)]
pub enum KernelError {
    /// The complete deterministic entry box does not admit a finite native image.
    #[error("kernel {function:?} domain at entry {entry}: {reason:?}")]
    Domain {
        /// Native function whose full-box premise failed.
        function: Elementwise,
        /// Logical column-order coordinate of the failed premise.
        entry: usize,
        /// Specific checked domain obstruction.
        reason: DomainReason,
    },
    /// Finite outward arithmetic could not establish the requested enclosure.
    #[error("kernel arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// Nonnegative bound construction or propagation failed.
    #[error("kernel bound: {0}")]
    Accuracy(#[from] nrz_accuracy::AccuracyFailure),
    /// A coefficient or closed operation disagrees with its checked shape.
    #[error("kernel coefficient or operation contract mismatch")]
    Contract,
    /// The current coverage vocabulary cannot combine different deterministic scopes.
    #[error("kernel inputs require the same deterministic coverage scope")]
    CoverageMismatch,
}

/// Private candidates cannot substitute the evidence of another closed kernel law.
#[cfg(test)]
mod tests {
    use crate::kernel_oracle::Rational;
    use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
    use crate::{
        Binding, ExecutionLimits, Extent, Heap, HeapLimits, KernelKind, OperationDraft,
        OperationError, RecipeEvidence, ShapeLimits,
    };
    use nrz_bindings::Descriptor;

    /// Same extents and finite payloads do not let a sum candidate claim a different recipe.
    /// ´test:unit:kernel-candidate-recipe-substitution-refuses´
    #[test]
    fn kernel_candidate_recipe_substitution_refuses() -> TestResult {
        let declarations = owner()?;
        let coordinate = axis(1)?;
        let contract = shape(&declarations, &coordinate, &coordinate)?;
        let draft = OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let limits = ExecutionLimits::default();
        let bound = Binding::new(
            draft.clone().check(limits)?,
            vec![source.value(), source.value()],
            limits,
        )
        .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let Some(RecipeEvidence::Kernel(claim)) = candidate.evidence.first_mut() else {
            return Err("kernel claim".into());
        };
        claim.kind = KernelKind::Subtract;
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("foreign recipe admitted")?;
        assert!(matches!(failure.cause(), OperationError::CandidateContract));
        assert_eq!(heap.descriptors.iter().count(), 1);
        let valid = execute(&mut heap, draft, vec![source.value(), source.value()])?;
        let (center, radius, error) = entry(&heap, output(&valid)?, 0)?;
        assert!(Rational::new(4, 1)?.covered(center, radius)?);
        assert!(Rational::new(4, 1)?.covered(center, error)?);
        Ok(())
    }
}
