// SPDX-License-Identifier: AGPL-3.0-only
//! Public policy and record fixtures for bounded solve recovery.

use crate::kernel_testing::TestResult;
use crate::test_api::{RecipeEvidence, SolveBudget, SolveEvidence, SolvePolicy};
use nrz_accuracy::{Budget, Limits, Promotion, RecoveryPolicy};
use nrz_bindings::Descriptor;

/// Bind a finite correction schedule to an explicit absolute output budget.
pub(crate) fn policy(
    tolerance: f64,
    refinements: u32,
    promotion: Promotion,
    limits: Limits,
) -> TestResult<SolvePolicy> {
    Ok(SolvePolicy::with_budget(SolveBudget::frobenius(
        Descriptor::ratio(),
        Budget::new(tolerance, 0.0, 1.0)?,
    )?)
    .with_recovery(RecoveryPolicy::new(refinements, 1, promotion)?, limits))
}
/// Inspect one retained solve theorem without manufacturing a success handle.
pub(crate) fn proof(evidence: &[RecipeEvidence]) -> TestResult<&SolveEvidence> {
    let Some(RecipeEvidence::Solve(claim)) = evidence.first() else {
        return Err("solve evidence".into());
    };
    Ok(claim)
}
