// SPDX-License-Identifier: AGPL-3.0-only
//! Public split-calculus syntax fixtures, compiled only in the public test target.

use super::budgets::derivation_budget as budget;
use super::cases::Fixture;
use super::common::TestResult;
use crate::test_api::domain::{AmbientDomain, RegionDraft, RegionExpr};
use crate::test_api::proof::{Judgment, SplitKind, SplitValue, Statement};
use crate::test_api::request::{SplitRange, Task};
use crate::test_api::syntax::{Draft, Operator, PredicateKind};
use crate::test_api::{Handle, Rational, Reference};

/// Exact requested bounds are caller data, not computed by the producer.
pub(crate) fn range(lower: &str, upper: &str) -> TestResult<SplitRange> {
    Ok(SplitRange {
        coordinate: "x".into(),
        lower: Rational::parse(lower, &mut budget()?)?,
        upper: Rational::parse(upper, &mut budget()?)?,
    })
}

/// Select the public task without constructing any proof evidence.
pub(crate) fn task(kind: SplitKind, lower: &str, upper: &str) -> TestResult<Task> {
    let bounds = Box::new(range(lower, upper)?);
    Ok(match kind {
        SplitKind::Integral => Task::Integrate(bounds),
        SplitKind::Sum => Task::Sum(bounds),
    })
}

/// Hand-written odd polynomial with an optional genuinely different upper branch.
pub(crate) fn odd(fixture: &mut Fixture, split: bool) -> TestResult<Handle> {
    let two = fixture.number("2")?;
    let product = fixture.term(Operator::Mul, &[two, fixture.x.clone()])?;
    let polynomial = fixture.term(Operator::Sub, &[product, fixture.one.clone()])?;
    if !split {
        return Ok(polynomial);
    }
    let four = fixture.number("4")?;
    let difference = fixture.term(Operator::Sub, &[fixture.x.clone(), four])?;
    let selector = fixture.predicate(PredicateKind::Positive, &[difference])?;
    let left = fixture.region(&selector, false)?;
    let right = fixture.region(&selector, true)?;
    fixture.cases(&[(left, polynomial.clone()), (right, polynomial)])
}

/// Construct the exact closed interval expected in a received theorem using public syntax.
pub(crate) fn interval(fixture: &mut Fixture, lower: &str, upper: &str) -> TestResult<Handle> {
    let low = fixture.number(lower)?;
    let high = fixture.number(upper)?;
    let start = fixture.term(Operator::Sub, &[fixture.x.clone(), low])?;
    let end = fixture.term(Operator::Sub, &[high, fixture.x.clone()])?;
    let first = fixture.predicate(PredicateKind::NonNegative, &[start])?;
    let second = fixture.predicate(PredicateKind::NonNegative, &[end])?;
    let base = fixture.region(&first, true)?;
    super::builders::admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(base),
                predicate: Reference::Existing(second),
            },
        }),
        &mut budget()?,
    )
}

/// Independent expected theorem has the full receiving context and explicit hand value.
pub(crate) fn expected(
    fixture: &mut Fixture,
    function: &Handle,
    kind: SplitKind,
    bounds: (&str, &str),
    value: &str,
) -> TestResult<Judgment> {
    let requested = range(bounds.0, bounds.1)?;
    let (low, high) = if requested
        .lower
        .compare(&requested.upper, &mut budget()?)?
        .is_gt()
    {
        (bounds.1, bounds.0)
    } else {
        bounds
    };
    let domain = interval(fixture, low, high)?;
    Ok(Judgment {
        context: fixture.context.clone(),
        layer: crate::test_api::domain::Layer::Pointwise,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Split(Box::new(SplitValue {
            kind,
            function: function.clone(),
            domain,
            range: requested,
            value: Rational::parse(value, &mut budget()?)?,
        })),
    })
}
