// SPDX-License-Identifier: AGPL-3.0-only
//! Public-API graph and covered-request construction shared by the fixture families.
//!
//! Compiled only in the public test target.

use crate::test_api::{
    AtomicSort, Binding, Budget, Cas, Declaration, Draft, Environment, ExpressionDraft, Field,
    Handle, Operation, Operator, Rational, Reference, Request, Scope, Sort,
};

use super::budgets::{derivation_budget, derivation_request_limits};
use super::common::TestResult;

/// Checked extraction of one setup handle.
pub(crate) fn at(handles: &[Handle], index: usize) -> TestResult<Handle> {
    handles
        .get(index)
        .cloned()
        .ok_or_else(|| "missing fixture handle".into())
}

/// Admit one checked draft without changing the caller's ledger or batch boundary.
pub(crate) fn admit(cas: &mut Cas, draft: Draft, budget: &mut Budget) -> TestResult<Handle> {
    at(&cas.admit(&[draft], budget)?, 0)
}

/// Declare ordered scalar coordinates in the requested mathematical field.
pub(crate) fn environment(field: Field, keys: &[&str]) -> Environment {
    let sort = Sort::Value(AtomicSort::Scalar(field.clone()));
    Environment {
        field,
        declarations: keys
            .iter()
            .map(|key| Declaration {
                key: (*key).into(),
                display_name: (*key).into(),
                sort: sort.clone(),
            })
            .collect(),
        ..Environment::default()
    }
}

/// Retain an exact expression draft, including its scope and ordered operands.
pub(crate) fn expression_draft(
    operator: Operator,
    sort: Sort,
    context: Reference,
    scope: Scope,
    binding: Option<Binding>,
    operands: &[Handle],
) -> Draft {
    Draft::Expression(ExpressionDraft {
        operator,
        sort,
        context,
        scope,
        binding,
        operands: operands.iter().cloned().map(Reference::Existing).collect(),
    })
}

/// Admit ordinary closed syntax through the same checked draft builder.
pub(crate) fn expression(
    cas: &mut Cas,
    context: &Handle,
    operator: Operator,
    sort: Sort,
    operands: &[Handle],
    budget: &mut Budget,
) -> TestResult<Handle> {
    admit(
        cas,
        expression_draft(
            operator,
            sort,
            Reference::Existing(context.clone()),
            Vec::new(),
            None,
            operands,
        ),
        budget,
    )
}

/// Parse a scalar and admit its term with separate fresh setup ledgers.
pub(crate) fn number(
    cas: &mut Cas,
    context: &Handle,
    sort: Sort,
    text: &str,
    fresh_budget: fn() -> TestResult<Budget>,
) -> TestResult<Handle> {
    let value = Rational::parse(text, &mut fresh_budget()?)?;
    expression(
        cas,
        context,
        Operator::Rational(value),
        sort,
        &[],
        &mut fresh_budget()?,
    )
}

/// Materialize and completely cover every ordered input before forming a request.
pub(crate) fn request(
    cas: &mut Cas,
    context: &Handle,
    operation: Operation,
    expressions: &[Handle],
) -> TestResult<Request> {
    let mut inputs = Vec::new();
    let mut receipts = Vec::new();
    for expression in expressions {
        let root = cas.materialize_derived(expression, expression, &mut derivation_budget()?)?;
        let receipt = cas.walk_derived(&root, &mut derivation_budget()?)?.receipt;
        inputs.push(root);
        receipts.push(receipt);
    }
    Ok(Request {
        operation,
        context: context.clone(),
        inputs,
        receipts,
        limits: derivation_request_limits(),
    })
}
