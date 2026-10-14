// SPDX-License-Identifier: AGPL-3.0-only
//! Public-API graph and covered-request construction shared by the fixture families.
//!
//! Compiled only in the public test target.

use crate::test_api::limits::Budget;
use crate::test_api::request::{CoveredInput, Request, Task};
use crate::test_api::syntax::{
    AtomicSort, Binding, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Scope,
    Sort,
};
use crate::test_api::{Cas, Handle, Rational, Reference};

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
    header: Reference,
    scope: Scope,
    binding: Option<Binding>,
    operands: &[Handle],
) -> Draft {
    Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort,
        header,
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
    let header = cas.context_header(context)?;
    admit(
        cas,
        expression_draft(
            operator,
            sort,
            Reference::Existing(header),
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
    task: Task,
    expressions: &[Handle],
) -> TestResult<Request> {
    let mut inputs = Vec::new();
    for expression in expressions {
        let root = cas.materialize_derived(expression, expression, &mut derivation_budget()?)?;
        let receipt = cas.walk_derived(&root, &mut derivation_budget()?)?.receipt;
        inputs.push(CoveredInput { root, receipt });
    }
    Ok(Request {
        task,
        context: context.clone(),
        inputs,
        premises: Vec::new(),
        limits: derivation_request_limits(),
    })
}

/// Build the header, ambient assignments region and root context as an explicit acyclic batch.
pub(crate) fn core_context(
    cas: &mut Cas,
    environment: Environment,
    assumptions: Vec<crate::test_api::domain::AssumptionDraft>,
    budget: &mut Budget,
) -> TestResult<Handle> {
    use crate::test_api::domain::{
        AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
    };
    let handles = cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(environment)),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions,
            }),
        ],
        budget,
    )?;
    at(&handles, 2)
}
