// SPDX-License-Identifier: AGPL-3.0-only
//! Original graph, source and occurrence fixtures.
//!
//! Compiled only in the public test target.

use crate::test_api::{
    Cas, Draft, Field, Handle, Limits, OccurrenceDraft, Operator, Reference, Sort, Span,
};

use super::budgets::graph_budget as budget;
use super::builders;
use super::common::{TestResult, real};

/// Admit one graph draft with an independent graph setup ledger.
pub(crate) fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    builders::admit(cas, draft, &mut budget()?)
}

/// Build a fresh immutable context with two distinct free identities.
pub(crate) fn fixture() -> TestResult<(Cas, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let environment = builders::environment(Field::Real, &["x", "y"]);
    let context = admit(
        &mut cas,
        Draft::Context {
            environment,
            assumptions: Vec::new(),
        },
    )?;
    Ok((cas, context))
}

/// Construct an ordinary closed expression with ordered operands.
pub(crate) fn expression(
    cas: &mut Cas,
    context: &Handle,
    operator: Operator,
    sort: Sort,
    operands: &[Handle],
) -> TestResult<Handle> {
    builders::expression(cas, context, operator, sort, operands, &mut budget()?)
}

/// Retain an original source occurrence, including absent precise spans.
pub(crate) fn occurrence(
    cas: &mut Cas,
    expression: &Handle,
    source: &Handle,
    span: Option<Span>,
    children: &[Handle],
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Occurrence(OccurrenceDraft {
            expression: Reference::Existing(expression.clone()),
            source: Some(Reference::Existing(source.clone())),
            span,
            children: children.iter().cloned().map(Reference::Existing).collect(),
            provenance: None,
            binder_names: Vec::new(),
        }),
    )
}

/// Build two leaf uses of one interned symbol beneath their original sum.
pub(crate) fn shared_original() -> TestResult<(Cas, Handle, Handle)> {
    let (mut cas, context) = fixture()?;
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "input".into(),
            text: "x+x".into(),
        },
    )?;
    let x = expression(&mut cas, &context, Operator::Free("x".into()), real(), &[])?;
    let sum = expression(
        &mut cas,
        &context,
        Operator::Add,
        real(),
        &[x.clone(), x.clone()],
    )?;
    let left = occurrence(&mut cas, &x, &source, Some(Span { start: 0, end: 1 }), &[])?;
    let right = occurrence(&mut cas, &x, &source, Some(Span { start: 2, end: 3 }), &[])?;
    let root = occurrence(
        &mut cas,
        &sum,
        &source,
        Some(Span { start: 0, end: 3 }),
        &[left, right],
    )?;
    Ok((cas, context, root))
}
