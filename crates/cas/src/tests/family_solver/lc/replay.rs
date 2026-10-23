// SPDX-License-Identifier: AGPL-3.0-only
//! Replay binds complete coverage to the independently reconstructed original LC request.

use crate::limits::Budget;
use crate::proof::{Judgment, Statement};
use crate::request::Task;
use crate::solving::{SolveStatement, StatementCoverage};
use crate::testing::common::TestResult;
use crate::testing::sorts::Fixture;

pub(super) fn expected(
    receiver: &Fixture,
    request: &crate::request::Request,
    set: &crate::Handle,
) -> TestResult<Judgment> {
    let Task::Solve(original) = &request.task else {
        return Err("LC original solve request".into());
    };
    Ok(Judgment {
        context: request.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: original.problem.clone(),
            set: set.clone(),
            region: receiver.cas.admissible_region(&request.context)?,
        }))),
    })
}

pub(super) fn fresh(
    source: &crate::Cas,
    set: &crate::Handle,
    bytes: &[u8],
    checking: &mut Budget,
) -> TestResult {
    let (mut receiver, request) = super::fixture::request([1, 1], true)?;
    let proposed = crate::testing::replay_answer::proposal(source, set, &mut receiver.cas)?;
    let expected = expected(&receiver, &request, &proposed)?;
    let witness = receiver.cas.replay(bytes, &expected, checking)?;
    assert!(witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        [] as [crate::domain::HypothesisRef; 0]
    );
    crate::testing::solve_blind::family_lc(&receiver.cas, &witness)?;
    assert_eq!(
        receiver
            .cas
            .export_witness(&witness, &mut super::ledger()?)?,
        bytes
    );
    Ok(())
}
