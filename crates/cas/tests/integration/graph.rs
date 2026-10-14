// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for graph.

use nrz_cas::limits::LimitsOverride;
use nrz_cas::request::{Outcome, Request, Task};
use nrz_cas::source::WalkEvent;
use nrz_cas::syntax::{AtomicSort, Binding, Draft, ExpressionDraft, Operator, Sort, Substitution};
use nrz_cas::{Rational, Reference};

use crate::testing::budgets::graph_budget as budget;
use crate::testing::common::{TestResult, real};
use crate::testing::graph::{admit, expression, fixture, occurrence};

/// Inserting a free symbol beneath a summation frame cannot capture it.
/// ´test:integration:binder-substitution´
#[test]
fn binder_substitution() -> TestResult {
    let (mut cas, context) = fixture()?;
    let syntax_header = cas.context_header(&context)?;
    let y = expression(&mut cas, &context, Operator::Free("y".into()), real(), &[])?;
    let body = admit(
        &mut cas,
        Draft::Expression(ExpressionDraft {
            space: None,
            operator: Operator::Free("x".into()),
            sort: real(),
            header: Reference::Existing(syntax_header.clone()),
            scope: vec![vec![AtomicSort::Integer]],
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let one = expression(
        &mut cas,
        &context,
        Operator::Rational(Rational::one()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        operator: Operator::Sum,
        sort: real(),
        header: Reference::Existing(syntax_header),
        scope: Vec::new(),
        binding: Some(Binding {
            sorts: vec![AtomicSort::Integer],
            body_operands: vec![0],
        }),
        operands: vec![
            Reference::Existing(body),
            Reference::Existing(one.clone()),
            Reference::Existing(one),
        ],
    });
    let sum = admit(&mut cas, draft.clone())?;
    let counts = cas.counts();
    assert_eq!(admit(&mut cas, draft)?, sum);
    assert_eq!(cas.counts(), counts);
    let replaced = cas.substitute(
        &sum,
        &[Substitution {
            key: "x".into(),
            replacement: y,
        }],
        &mut budget()?,
    )?;
    let operands = cas.operands(&replaced, &mut budget()?)?;
    let body = operands.first().ok_or("sum lost its body")?;
    assert_eq!(cas.expression(body)?.0, &Operator::Free("y".into()));
    let root = cas.materialize_derived(&replaced, &sum, &mut budget()?)?;
    let walk = cas.walk_derived(&root, &mut budget()?)?;
    assert!(matches!(
        walk.events.first(),
        Some(WalkEvent::EnterBinder { .. })
    ));
    assert_eq!(walk.receipt.visited(), 4);
    Ok(())
}

/// Zero multiplication still visits the partial quotient and both of its original uses.
/// ´test:integration:no-normalization-before-coverage´
#[test]
fn no_normalization_before_coverage() -> TestResult {
    let (mut cas, context) = fixture()?;
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "partial".into(),
            text: "0*(x/x)".into(),
        },
    )?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let zero = expression(
        &mut cas,
        &context,
        Operator::Rational(Rational::zero()),
        real(),
        &[],
    )?;
    let quotient = expression(
        &mut cas,
        &context,
        Operator::Div,
        real(),
        &[x.clone(), x.clone()],
    )?;
    let product = expression(
        &mut cas,
        &context,
        Operator::Mul,
        real(),
        &[zero.clone(), quotient.clone()],
    )?;
    let numerator = occurrence(&mut cas, &x, &source, None, &[])?;
    let denominator = occurrence(&mut cas, &x, &source, None, &[])?;
    let quotient = occurrence(
        &mut cas,
        &quotient,
        &source,
        None,
        &[numerator, denominator],
    )?;
    let zero = occurrence(&mut cas, &zero, &source, None, &[])?;
    let root = occurrence(&mut cas, &product, &source, None, &[zero, quotient])?;
    let receipt = cas.walk_original(&root, &mut budget()?)?.receipt;
    assert_eq!(receipt.visited(), 5);
    let outcome = cas.run(&Request {
        task: Task::Simplify,
        context,
        inputs: vec![nrz_cas::request::CoveredInput { root, receipt }],
        premises: Vec::new(),
        limits: LimitsOverride::default(),
    });
    assert!(matches!(outcome, Outcome::Conditional { .. }));
    Ok(())
}
