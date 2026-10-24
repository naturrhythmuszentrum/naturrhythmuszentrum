// SPDX-License-Identifier: AGPL-3.0-only
//! The admitted grammar preserves each original family body, sign and divisor.

use super::{AT, Error};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    solving::{FamilyArgumentRole, SolveView},
    source::{SyntaxDescriptor, SyntaxNode, SyntaxRole},
    syntax::{AtomicSort, Field, Operator, PredicateKind, Sort},
};

/// One exact edge is selected by role, never by display name or a numerical value.
pub(super) fn dependency(syntax: &SyntaxDescriptor, role: SyntaxRole) -> Result<&Handle, Error> {
    syntax
        .dependencies
        .iter()
        .find(|edge| edge.role == role)
        .map(|edge| &edge.target)
        .ok_or(Error::Structure)
}

/// Closed conditions retain their complete original operand inventory.
pub(super) fn condition(
    cas: &Cas,
    node: &Handle,
    kind: &PredicateKind,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Error> {
    let syntax = cas.describe_syntax(node, budget)?;
    if !syntax.scope.is_empty()
        || syntax.binding.is_some()
        || !matches!(syntax.node, SyntaxNode::Condition(actual) if &actual == kind)
    {
        return Err(Error::Structure);
    }
    let mut values = super::super::super::support::reserve(syntax.dependencies.len(), budget)?;
    for (index, edge) in syntax.dependencies.into_iter().enumerate() {
        if edge.role != SyntaxRole::Operand(u32::try_from(index).map_err(|_| Error::Structure)?) {
            return Err(Error::Structure);
        }
        values.push(edge.target);
    }
    Ok(values)
}

/// Scalar nodes have no hidden scopes, attachments, domains or nonoperand children.
pub(super) fn operands(
    cas: &Cas,
    node: &Handle,
    field: Field,
    operator: &Operator,
    count: usize,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Error> {
    let syntax = cas.describe_syntax(node, budget)?;
    let SyntaxNode::Expression(value) = syntax.node else {
        return Err(Error::Structure);
    };
    if value.sort != Sort::Value(AtomicSort::Scalar(field))
        || &value.operator != operator
        || !syntax.scope.is_empty()
        || syntax.binding.is_some()
        || syntax.dependencies.len() != count
    {
        return Err(Error::Structure);
    }
    let mut values = super::super::super::support::reserve(count, budget)?;
    for (index, edge) in syntax.dependencies.into_iter().enumerate() {
        if edge.role != SyntaxRole::Operand(u32::try_from(index).map_err(|_| Error::Structure)?) {
            return Err(Error::Structure);
        }
        values.push(edge.target);
    }
    Ok(values)
}

/// Literal zero is checked before any cancellation; the numerator is exactly one.
pub(super) fn literal(
    cas: &Cas,
    node: &Handle,
    field: Field,
    one: bool,
    budget: &mut Budget,
) -> Result<(), Error> {
    let value = if one {
        nrz_cas::Rational::one()
    } else {
        nrz_cas::Rational::zero()
    };
    operands(cas, node, field, &Operator::Rational(value), 0, budget)?;
    Ok(())
}

/// Only the original declared input can discharge this reciprocal's domain obligation.
pub(super) fn reciprocal(
    cas: &Cas,
    node: &Handle,
    input: &Handle,
    budget: &mut Budget,
) -> Result<(), Error> {
    let values = operands(cas, node, Field::Real, &Operator::Div, 2, budget)?;
    literal(cas, &values[0], Field::Real, true, budget)?;
    if &values[1] != input {
        return Err(Error::Structure);
    }
    Ok(())
}

/// The sole original association retains its exact region and placement-bearing body.
fn body(cas: &Cas, node: &Handle, region: &Handle, budget: &mut Budget) -> Result<Handle, Error> {
    budget.work(1, AT)?;
    if matches!(cas.solve_view(node), Ok(SolveView::FamilyExpression(_))) {
        return Ok(node.clone());
    }
    let syntax = cas.describe_syntax(node, budget)?;
    if !matches!(&syntax.node, SyntaxNode::Expression(value) if value.operator == Operator::Cases)
        || !syntax.scope.is_empty()
        || syntax.binding.is_some()
        || syntax.dependencies.len() != 2
        || dependency(&syntax, SyntaxRole::Domain)? != region
    {
        return Err(Error::Structure);
    }
    let association = cas.describe_syntax(dependency(&syntax, SyntaxRole::Arm(0))?, budget)?;
    if !matches!(association.node, SyntaxNode::CaseArm)
        || dependency(&association, SyntaxRole::Region)? != region
    {
        return Err(Error::Structure);
    }
    Ok(dependency(&association, SyntaxRole::Value(0))?.clone())
}

/// Local arguments retain their Real-base versus Complex-fibre placement roles.
pub(super) fn local(
    cas: &Cas,
    node: &Handle,
    scope: (&Handle, &Handle),
    operator: &Operator,
    roles: [FamilyArgumentRole; 2],
    budget: &mut Budget,
) -> Result<[Handle; 2], Error> {
    let node = body(cas, node, scope.1, budget)?;
    budget.work(1, AT)?;
    let SolveView::FamilyExpression(data) = cas.solve_view(&node)? else {
        return Err(Error::Structure);
    };
    let [left, right] = data.arguments.as_slice() else {
        return Err(Error::Structure);
    };
    if &data.family != scope.0
        || &data.region != scope.1
        || &data.operator != operator
        || data.result != Sort::Value(AtomicSort::Scalar(Field::Complex))
        || !data.scope.is_empty()
        || [left.role, right.role] != roles
    {
        return Err(Error::Structure);
    }
    Ok([left.value.clone(), right.value.clone()])
}
