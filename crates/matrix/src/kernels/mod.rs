// SPDX-License-Identifier: AGPL-3.0-only
//! Closed deterministic region kernels under the existing transaction owner.

mod arithmetic;
mod backend_product;
pub(crate) mod contract;
mod contributions;
mod draft;
mod evaluate;
pub(crate) mod linear;
mod linear_evaluate;
pub(crate) mod lowering;
mod nonlinear;
mod product;

pub(crate) use contract::KernelContract;
pub use contract::{KernelEvidence, KernelKind};
pub use contributions::ProductTerm;
pub(crate) use contributions::{
    ContributionReservation, finish as finish_contributions, views as contribution_views,
};
pub use linear::{FixedCoefficient, LinearMap};
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
    use nrz_units::Descriptor;

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
