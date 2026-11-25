// SPDX-License-Identifier: AGPL-3.0-only
//! The sole registered placement has an exact local incidence signature.

use super::ScalarPieceData;
use super::admission::{invalid, object};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId, State, Vertex};
use crate::model::{AtomicSort, Field, Operator, PredicateKind};
use crate::solving::SolveDraft;

/// Check Im(q)=0 and Re(q)=t in the declared local Complex/Real frame.
pub(super) fn validate(
    state: &State,
    value: &ScalarPieceData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::SpacePlacement(placement) = object(state, &value.placement, budget)? else {
        return Err(invalid());
    };
    super::registry::placement(state, placement, budget)?;
    let region = state.region(value.region.logical_id(), budget)?;
    let (header, scope) =
        crate::descriptor::metadata(state, value.local_frame.logical_id(), budget)?;
    let mut expected = region.scope.clone();
    expected.push(vec![
        AtomicSort::Scalar(Field::Complex),
        AtomicSort::Scalar(Field::Real),
    ]);
    if value.piece != 0 || header != region.header || scope != &expected {
        return Err(invalid());
    }
    let clauses = predicate(
        state,
        value.local_frame.logical_id(),
        &PredicateKind::And,
        budget,
    )?;
    let [imaginary, real] = clauses else {
        return Err(invalid());
    };
    let imaginary = predicate(state, *imaginary, &PredicateKind::Equal, budget)?;
    let real = predicate(state, *real, &PredicateKind::Equal, budget)?;
    let [component, zero] = imaginary else {
        return Err(invalid());
    };
    let query = component_query(state, *component, &Operator::ImagPart, budget)?;
    if !matches!(&state.expression(*zero, budget)?.operator, Operator::Rational(value) if value.is_zero())
    {
        return Err(invalid());
    }
    let [component, local] = real else {
        return Err(invalid());
    };
    if component_query(state, *component, &Operator::RealPart, budget)? != query
        || state.expression(*local, budget)?.operator != (Operator::Bound { depth: 0, slot: 1 })
    {
        return Err(invalid());
    }
    Ok(())
}

/// Read only the exact registered incidence predicate constructor.
fn predicate<'a>(
    state: &'a State,
    id: LogicalId,
    expected: &PredicateKind,
    budget: &mut Budget,
) -> Result<&'a [LogicalId], Fault> {
    match state.node(id, budget)? {
        Vertex::Condition(value) if value.kind == *expected => Ok(&value.arguments),
        Vertex::Expression(value) if value.operator == Operator::Predicate(expected.clone()) => {
            Ok(&value.operands)
        }
        _ => Err(invalid()),
    }
}

/// Require a component of the exact local Complex query slot.
fn component_query(
    state: &State,
    id: LogicalId,
    expected: &Operator,
    budget: &mut crate::budget::Budget,
) -> Result<LogicalId, Fault> {
    let term = state.expression(id, budget)?;
    let [query] = term.operands.as_slice() else {
        return Err(invalid());
    };
    if term.operator != *expected
        || state.expression(*query, budget)?.operator != (Operator::Bound { depth: 0, slot: 0 })
    {
        return Err(invalid());
    }
    Ok(*query)
}
