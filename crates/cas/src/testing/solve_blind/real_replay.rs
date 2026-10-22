// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh replay rejects a changed original sort, base region or strict original denominator.

use crate::budget::{Budget, Limits};
use crate::graph::{Draft, Handle, Reference};
use crate::model::Field;
use crate::proof_model::{Judgment, Statement};
use crate::solving::{SolveStatement, StatementCoverage};
use crate::testing::common::TestResult;

/// Produce the universal theorem once, then alter only independently reconstructed expected claims.
pub(crate) fn refusals() -> TestResult {
    let (mut source, request) = super::real_acceptance::request()?;
    let outcome = source.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("Real expected baseline: {outcome:?}").into());
    };
    let proof = answer.coverage.ok_or("Real expected coverage")?;
    let bytes = source
        .cas
        .export_witness(&proof, &mut Budget::new(Limits::blind_cubic())?)?;
    for mutation in 0..3 {
        changed(&bytes, mutation)?;
    }
    Ok(())
}

/// The expected declaration sort is chosen before importing the Real witness.
fn changed(bytes: &[u8], mutation: usize) -> TestResult {
    let field = if mutation == 0 {
        Field::Complex
    } else {
        Field::Real
    };
    let (mut fixture, request) = super::source::symbolic_in_field(&["d", "c", "b", "a"], &field)?;
    let crate::task::Task::Solve(original) = &request.task else {
        return Err("Real expected request".into());
    };
    let imported = fixture
        .cas
        .inspect(bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("Real expected solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("Real expected coverage claim".into());
    };
    let mut expected = StatementCoverage {
        problem: original.problem.clone(),
        set: claim.set.clone(),
        region: fixture.region.clone(),
    };
    match mutation {
        0 => {
            assert_ne!(expected.problem, claim.problem);
            assert_ne!(fixture.context, imported.judgment().context);
        }
        1 => {
            assert_eq!(expected.region, claim.region);
            expected.region = empty_base(&mut fixture)?;
            assert_ne!(expected.region, claim.region);
        }
        2 => {
            assert_eq!(expected.problem, claim.problem);
            expected.problem = denominator(&mut fixture, &original.problem)?;
            assert_ne!(expected.problem, claim.problem);
        }
        _ => return Err("Real expected mutation".into()),
    }
    let expected = Judgment {
        context: fixture.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(expected))),
    };
    assert!(
        fixture
            .cas
            .replay(bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)
            .is_err()
    );
    Ok(())
}

/// The base domain is a separately admitted exact region in the same declaration frame.
fn empty_base(fixture: &mut super::source::Fixture) -> TestResult<Handle> {
    let draft = Draft::Region(crate::domain_model::RegionDraft {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        ambient: crate::domain_model::AmbientDomain::Product {
            coordinates: fixture.coefficients.clone(),
        },
        expression: crate::domain_model::RegionExpr::Empty,
    });
    fixture
        .cas
        .admit(&[draft], &mut Budget::new(Limits::blind_cubic())?)?
        .into_iter()
        .next()
        .ok_or_else(|| "Real changed base".into())
}

/// An additional original divisor changes the problem even though its covered polynomial is unchanged.
fn denominator(fixture: &mut super::source::Fixture, original: &Handle) -> TestResult<Handle> {
    let problem = crate::solving::elementary::proof::problem(&fixture.cas, original)?;
    assert_eq!(problem.denominators, [] as [Handle; 0]);
    let predicate = problem.predicate.clone();
    let divisor = fixture.free("a")?;
    fixture.problem("x", &predicate, &[divisor])
}
