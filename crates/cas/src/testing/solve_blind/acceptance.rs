// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh expected claims and independent traversal retain the original symbolic source.

use crate::graph::{Cas, Handle};
use crate::limits::{Budget, Limits};
use crate::proof_model::{Judgment, Statement};
use crate::solving::{
    SolveAnswer, SolveForm, SolveStatement, StatementCoverage, StatementPresentation,
};
use crate::testing::common::TestResult;

type Solved = (super::source::Fixture, crate::request::Request, SolveAnswer);

/// Search receives only the original symbolic source and its fixed acceptance limits.
pub(crate) fn solved(keys: &[&str]) -> TestResult<Solved> {
    let (mut fixture, request) = super::source::symbolic(keys)?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("symbolic acceptance: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    assert_eq!(answer.unresolved_regions, [] as [Handle; 0]);
    assert!(
        answer
            .coverage
            .as_ref()
            .is_some_and(crate::proof::VerifiedWitness::is_discharged)
    );
    Ok((fixture, request, answer))
}

/// Expected source problem and region are built before importing either claimed theorem.
pub(crate) fn fresh(cas: &Cas, answer: &SolveAnswer, keys: &[&str]) -> TestResult {
    let (mut fixture, request) = super::source::symbolic(keys)?;
    let crate::request::Task::Solve(original) = &request.task else {
        return Err("fresh source".into());
    };
    let coverage = answer.coverage.as_ref().ok_or("fresh coverage")?;
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let imported = fixture
        .cas
        .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("fresh solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("fresh coverage claim".into());
    };
    let set = claim.set.clone();
    let expected = judgment(
        &fixture,
        SolveStatement::Coverage(StatementCoverage {
            problem: original.problem.clone(),
            set: set.clone(),
            region: fixture.region.clone(),
        }),
    );
    replay(&mut fixture.cas, &bytes, &expected)?;
    let presentation = answer.facts.last().ok_or("fresh presentation")?;
    let bytes = cas.export_witness(presentation, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = judgment(
        &fixture,
        SolveStatement::Presentation(StatementPresentation {
            set,
            form: SolveForm::Radicals,
            region: fixture.region.clone(),
        }),
    );
    replay(&mut fixture.cas, &bytes, &expected)
}

fn judgment(fixture: &super::source::Fixture, claim: SolveStatement) -> Judgment {
    Judgment {
        context: fixture.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(claim)),
    }
}

fn replay(cas: &mut Cas, bytes: &[u8], expected: &Judgment) -> TestResult {
    let checked = cas.replay(bytes, expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(checked.is_discharged());
    assert_eq!(
        checked.hypotheses(),
        [] as [crate::domain::HypothesisRef; 0]
    );
    assert_eq!(
        cas.export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Both original declaration frames now reach independent fresh expected-claim replay.
pub(crate) fn source() -> TestResult {
    for keys in [["d", "c", "b", "a"], ["delta", "gamma", "beta", "alpha"]] {
        let (fixture, _, answer) = solved(&keys)?;
        fresh(&fixture.cas, &answer, &keys)?;
    }
    Ok(())
}
