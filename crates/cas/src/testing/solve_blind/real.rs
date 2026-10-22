// SPDX-License-Identifier: AGPL-3.0-only
//! Real proposal replay attributes failures and retains independent cumulative ledger observations.

use crate::budget::{Budget, Limits};
use crate::testing::common::TestResult;

/// Replay the complete original Real request before any diagnostic prefix is inspected.
pub(crate) fn prefixes() -> TestResult {
    let (mut fixture, mut request) =
        super::source::symbolic_in_field(&["d", "c", "b", "a"], &crate::model::Field::Real)?;
    let crate::task::Task::Solve(specification) = &mut request.task else {
        return Err("Real request".into());
    };
    specification.form = crate::solving::SolveForm::AnyExact;
    let mut audit = crate::solving::radicals::route::Audit::default();
    let mut generation = Budget::new(Limits::blind_cubic())?;
    let proposed = crate::solving::solve_search::propose(
        &mut fixture.cas,
        &request,
        &mut audit,
        &mut generation,
    )
    .map_err(|error| {
        format!(
            "Real producer: {error:?}; last phase {:?}; usage {:?}",
            audit.phases.last(),
            generation.usage()
        )
    })?;
    let construction = proposed.construction;
    let mut checker = Budget::new(Limits::blind_cubic())?;
    match fixture
        .cas
        .check_proof(&construction.context, &construction.steps, &mut checker)
    {
        Ok(proof) => {
            assert!(proof.is_discharged());
            Ok(())
        }
        Err(error) => rejected(&mut fixture.cas, &construction.steps, &error),
    }
}

/// Diagnostics run only after the whole unmodified proof has failed and never count as acceptance.
fn rejected(
    cas: &mut crate::Cas,
    steps: &[crate::proof_model::ApplicationDraft],
    error: &crate::Fault,
) -> TestResult {
    for (index, step) in steps.iter().enumerate() {
        let prefix = super::trace::closure(steps, index)?;
        cas.check_proof(&step.context, &prefix, &mut Budget::new(Limits::blind_cubic())?)
            .map_err(|fault| format!("Real first rejected application {index}: {fault:?}; rule {:?}; conclusion {:?}; certificate {:?}",
                step.rule, step.conclusion, step.certificate))?;
    }
    Err(format!("Real whole proof failed after accepted dependency closures: {error:?}").into())
}
