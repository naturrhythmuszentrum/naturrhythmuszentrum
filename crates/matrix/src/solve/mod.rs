// SPDX-License-Identifier: AGPL-3.0-only
//! Square solve candidates and independent certificates under the transaction lease.

mod backend;
mod contract;
mod error;
mod evaluate;
mod evidence;
mod factor;
mod inputs;
mod policy;
mod restoration;
pub(crate) mod sensitivity;
pub(crate) use inputs::RightHandSide;
pub use policy::{SolveBudget, SolvePolicy};
pub use sensitivity::SensitivityEvidence;

pub(crate) use contract::SolveContract;
pub use error::SolveError;
pub use evidence::SolveEvidence;
pub use factor::FactorRef;

#[cfg(test)]
mod tests {
    use crate::kernel_oracle::Rational;
    use crate::solve_testing::{TestResult, draft};
    use crate::{
        Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, RecipeEvidence,
    };
    use nrz_units::Descriptor;

    /// Altered proof extent cannot authorize otherwise finite private solve payloads.
    /// ´test:unit:solve-candidate-proof-substitution-refuses-atomically´
    #[test]
    fn solve_candidate_proof_substitution_refuses_atomically() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let limits = ExecutionLimits::default();
        let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
            .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let payload = candidate.outputs.first().ok_or("payload")?;
        let center = *payload.centers.first().ok_or("center")?;
        let radius = *payload
            .bounds
            .as_ref()
            .and_then(|values| values.first())
            .ok_or("bound")?;
        assert!(Rational::new(1, 1)?.covered(center, radius)?);
        let Some(RecipeEvidence::Solve(claim)) = candidate.evidence.first_mut() else {
            return Err("solve proof".into());
        };
        claim.extent = Extent::new(1, 2)?;
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("corruption admitted")?;
        assert!(matches!(failure.cause(), OperationError::CandidateContract));
        assert_eq!(heap.descriptors.iter().count(), 1);
        Ok(())
    }
}
