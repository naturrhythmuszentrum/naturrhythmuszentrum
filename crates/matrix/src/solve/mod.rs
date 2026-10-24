// SPDX-License-Identifier: AGPL-3.0-only
//! Square solve candidates and independent certificates under the transaction lease.
//!
//! # Square solve admission and acceptance
//!
//! `OperationDraft::solve` checks a positive square `A: X → Y`, a nonempty `B: W → Y`, and a concrete homogeneous output shape on `W → X`. Every ordered axis and output scalar pairing is checked before binding. `Binding::validate` requires a complete joint model for statistical operands and charges the complete factor, candidate, verification and scratch allowance before backend entry. `CheckedOperation::solve_work` exposes that conservative requirement. Represented and declared-bound solves preserve their named scope; mixed scopes refuse. The complete chosen input totals include previously carried numerical error.
//!
//! `OperationDraft::inverse` derives the typed identity on the original equation axis and publishes a separately checked `Y → X` result. Its physical source closure contains only the operator. The candidate has its own residual against that identity and its own common-column total, independently of the private witness used for ordinary solves. Results require concrete homogeneous output scalar kinds; mixed-entry execution and arbitrary per-entry restoration remain outside the current operation subset.
//!
//! The public acceptance lifecycles combine the committed static truss and common-source resistor ladder with shape checking, independent solve certificates, implicit sensitivities, complete covariance and recorded publication. The truss keeps stiffness, force and displacement descriptors. The ladder exactly eliminates the known source voltage and source current, solves the reduced three-coordinate nodal system, and reconstructs all five original outputs. It restores voltage and current into separate homogeneous regions, materializes all nine cross-point covariance blocks and computes an ordered mean. The full five-by-five statistical solve remains an asserted symbolic work-budget refusal; the current solver creates that budget internally without a public override. Direct mixed-region numerical admission remains an explicit refusal; checked mixed shapes and normalization metadata do not imply mixed-entry execution.
//!
//! `cargo test -p nrz-matrix --test public integration::acceptance` checks exact rational targets, refusal stages and charged work/storage counts. `cargo run -p nrz-matrix --locked --example acceptance_measurements -- truss` prints the shared fixture's observed timings and public evidence; replace `truss` with `resistor`, `certificate-edge`, `shared-a-b`, `stale-factor` or `bounded-refinement` for the other committed cases. The measurements in `docs/records/matrix-core-acceptance.md` name their release-profile environment; they do not describe runs using the workspace dev profile (`opt-level = 2`). Tests do not print measurements or assert runtime thresholds. Region payload counts exclude backend padding and allocator overhead; server peak resident memory is recorded separately. The truss's illustrative moment model and its deterministic import enclosure are separate conditional scenarios, and a covariance never supplies its deterministic radius.

mod allocations;
pub use allocations::{FactorAllocation, FactorAllocationStage, FactorInventory};
mod backend;
mod contract;
mod error;
pub(crate) mod evaluate;
mod evidence;
mod factor;
mod inputs;
mod lifecycle;
pub use lifecycle::{SolveFailure, SolveFailureRecord, SolveRecordKeys, SolveStage};
mod policy;
pub(crate) mod recovery;
mod restoration;
mod retry;
pub use recovery::{SolveAttempt, SolveRecoveryEvidence, SolveStop};
pub use retry::{RecoveredSolve, RecoveryFailure};
pub(crate) mod sensitivity;
pub(crate) use inputs::RightHandSide;
pub use policy::{SolveBudget, SolvePolicy};
pub use sensitivity::SensitivityEvidence;

pub(crate) use contract::SolveContract;
pub use error::SolveError;
pub use evidence::SolveEvidence;
pub use factor::{FactorDependency, FactorRef, FactorUse};

#[cfg(test)]
mod tests {
    use crate::kernel_oracle::Rational;
    use crate::solve_testing::{TestResult, draft};
    use crate::{
        Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, RecipeEvidence,
    };
    use nrz_bindings::Descriptor;

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
