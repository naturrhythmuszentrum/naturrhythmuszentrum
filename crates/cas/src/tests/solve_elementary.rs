// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped interaction checks for independently replayed elementary rules.

use crate::testing::common::TestResult;

use crate::budget::{Budget, Limits};
use crate::graph::{Draft, Reference};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{ApplicationDraft, Certificate, DomainContract, Statement};
use crate::solving::{
    CertificateMembership, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    SolveSubstitution, SolveView, StatementMember, TupleData,
};
use crate::testing::solve_graph::{Fixture, admit, expression};

/// A checked membership keeps an unrelated conditional premise's full scoped residual.
/// ´test:crate:elementary-preserves-residuals´
#[test]
fn elementary_preserves_residuals() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (cancel_step, one) = cancellation(&mut fixture)?;
    let region = fixture.cas.admissible_region(&fixture.context)?;
    let member_step = membership(&mut fixture, one, region)?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &[cancel_step, member_step],
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(checked.judgment().residuals.len(), 1);
    assert_eq!(checked.hypotheses(), []);
    Ok(())
}

/// A scalar guard is created by the existing independently replayed cancellation rule.
fn cancellation(
    fixture: &mut crate::testing::solve_graph::Fixture,
) -> TestResult<(ApplicationDraft, crate::graph::Handle)> {
    let scalar = Sort::Value(AtomicSort::Scalar(Field::Real));
    let x = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Free("x".into()),
            scalar.clone(),
            Vec::new(),
        ),
    )?;
    let one = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Rational(crate::number::Rational::one()),
            scalar.clone(),
            Vec::new(),
        ),
    )?;
    let quotient = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Div,
            scalar,
            vec![Reference::Existing(x.clone()), Reference::Existing(x)],
        ),
    )?;
    let region = fixture.cas.admissible_region(&fixture.context)?;
    let cancellation = ApplicationDraft {
        rule: crate::rule::Primitive::Cancel,
        context: fixture.context.clone(),
        conclusion: Statement::Equality {
            left: quotient,
            right: one.clone(),
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    Ok((cancellation, one))
}

/// Membership in the identity predicate depends on the scalar premise without dropping its guard.
fn membership(
    fixture: &mut crate::testing::solve_graph::Fixture,
    one: crate::graph::Handle,
    region: crate::graph::Handle,
) -> TestResult<ApplicationDraft> {
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("missing problem".into());
    };
    let coordinates = problem.unknowns.clone();
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates: Reference::Existing(coordinates),
            entries: vec![Reference::Existing(one.clone())],
        })),
    )?;
    let membership = ApplicationDraft {
        rule: crate::rule::Primitive::Solve(SolveSchema::Membership),
        context: fixture.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::Member(StatementMember {
            problem: fixture.problem.clone(),
            tuple: tuple.clone(),
            region: region.clone(),
        }))),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::Membership(
            CertificateMembership {
                problem: fixture.problem.clone(),
                tuple,
                region,
                substitutions: vec![SolveSubstitution {
                    key: "x".into(),
                    expression: one,
                }],
                definedness: Vec::new(),
            },
        ))),
    };
    Ok(membership)
}
