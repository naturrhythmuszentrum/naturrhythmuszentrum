// SPDX-License-Identifier: AGPL-3.0-only
//! Regional sign fixtures, compiled only in the public test target.

use super::builders::admit;
use super::common::{TestResult, real};
use super::readings::{Fixture, ledger};
use crate::test_api::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::test_api::proof::{Judgment, Statement};
use crate::test_api::solving::{RealSignConclusion, SolveStatement, StatementRealSign};
use crate::test_api::syntax::{Draft, Operator, PredicateKind};
use crate::test_api::{Handle, Rational, Reference};

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public strict sign fixtures"
)]
pub(crate) fn guarded(fixture: &mut Fixture, subject: &Handle, sign: i8) -> TestResult {
    if sign == 0 {
        return Ok(());
    }
    let subject = if sign < 0 {
        fixture.term(Operator::Neg, real(), std::slice::from_ref(subject))?
    } else {
        subject.clone()
    };
    let header = fixture.cas.context_header(&fixture.context)?;
    let base = fixture.cas.admissible_region(&fixture.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind: if sign == 2 {
                PredicateKind::NonNegative
            } else {
                PredicateKind::Positive
            },
            arguments: vec![Reference::Existing(subject)],
        },
        &mut ledger()?,
    )?;
    let region = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(base),
                predicate: Reference::Existing(predicate),
            },
        }),
        &mut ledger()?,
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(region),
            assumptions: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public strict sign fixtures"
)]
pub(crate) fn fixture(first: i8, second: i8) -> TestResult<(Fixture, Vec<Handle>)> {
    let mut fixture = Fixture::new(&[("L", real()), ("C", real())])?;
    let left = fixture.symbol("L")?;
    let right = fixture.symbol("C")?;
    guarded(&mut fixture, &left, first)?;
    guarded(&mut fixture, &right, second)?;
    let one = fixture.term(Operator::Rational(Rational::one()), real(), &[])?;
    let product = fixture.term(Operator::Mul, real(), &[left.clone(), right.clone()])?;
    let quotient = fixture.term(Operator::Div, real(), &[left, right])?;
    let reciprocal = fixture.term(Operator::Div, real(), &[one, product.clone()])?;
    let negative = fixture.term(Operator::Neg, real(), std::slice::from_ref(&reciprocal))?;
    Ok((fixture, vec![product, quotient, reciprocal, negative]))
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public strict sign fixtures"
)]
pub(crate) fn expected(
    fixture: &Fixture,
    subject: &Handle,
    positive: bool,
) -> TestResult<Judgment> {
    Ok(Judgment {
        context: fixture.context.clone(),
        layer: crate::test_api::domain::Layer::Logic,
        space: None,
        statement: Statement::Solve(Box::new(SolveStatement::RealSign(StatementRealSign {
            subject: subject.clone(),
            region: fixture.cas.admissible_region(&fixture.context)?,
            conclusion: if positive {
                RealSignConclusion::Positive
            } else {
                RealSignConclusion::Negative
            },
        }))),
        residuals: Vec::new(),
    })
}
