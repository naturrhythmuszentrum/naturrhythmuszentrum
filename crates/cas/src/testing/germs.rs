// SPDX-License-Identifier: AGPL-3.0-only
//! Public germ syntax and independent expected judgments, used only by public tests.

use super::budgets::derivation_budget as budget;
use super::cases::Fixture;
use super::common::TestResult;
use crate::test_api::domain::{Layer, Side};
use crate::test_api::proof::{GermAt, GermMode, GermValue, Judgment, Statement};
use crate::test_api::request::{SeriesAt, Task};
use crate::test_api::syntax::Operator;
use crate::test_api::{Handle, Rational};

/// Explicit centre and approach, with no point ownership assertion.
pub(crate) fn at(fixture: &mut Fixture, centre: &str, side: Option<Side>) -> TestResult<GermAt> {
    let point = fixture.number(centre)?;
    Ok(GermAt {
        coordinate: "x".into(),
        point: fixture.boundary(&point)?,
        side,
    })
}

/// Construct the exact public task from caller intent.
pub(crate) fn task(at: &GermAt, mode: GermMode) -> Task {
    match mode {
        GermMode::Limit => Task::Limit(Box::new(at.clone())),
        GermMode::Series { order, analytic } => Task::Series(Box::new(SeriesAt {
            at: at.clone(),
            order,
            analytic,
        })),
    }
}

/// Expected coefficients are supplied as hand-derived exact literals.
pub(crate) fn expected(
    fixture: &Fixture,
    function: &Handle,
    at: &GermAt,
    mode: GermMode,
    numbers: &[&str],
) -> TestResult<Judgment> {
    let mut ledger = budget()?;
    let coefficients = numbers
        .iter()
        .map(|value| Rational::parse(value, &mut ledger))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Judgment {
        context: fixture.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Germ(Box::new(GermValue {
            function: function.clone(),
            at: at.clone(),
            mode,
            coefficients,
        })),
    })
}

/// A cubic with hand expansion 3 + 5h + 3h² + h³ at centre one.
pub(crate) fn cubic(fixture: &mut Fixture) -> TestResult<Handle> {
    let x = fixture.x.clone();
    let cube = fixture.term(Operator::IntegerPower(3), std::slice::from_ref(&x))?;
    let two = fixture.number("2")?;
    let linear = fixture.term(Operator::Mul, &[two, x])?;
    fixture.term(Operator::Add, &[cube, linear])
}
