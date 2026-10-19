// SPDX-License-Identifier: AGPL-3.0-only
//! The sole registered placement has an exact local incidence signature.

use super::ScalarPieceData;
use super::admission::{invalid, object};
use crate::error::Fault;
use crate::graph::{Handle, LogicalId, State, Vertex};
use crate::model::{AtomicSort, Field, Operator, PredicateKind};
use crate::solving::SolveDraft;

/// Check Im(q)=0 and Re(q)=t in the declared local Complex/Real frame.
pub(super) fn validate(state: &State, value: &ScalarPieceData<Handle>) -> Result<(), Fault> {
    let SolveDraft::SpacePlacement(placement) = object(state, &value.placement)? else {
        return Err(invalid());
    };
    super::registry::placement(state, placement)?;
    let region = state.region(value.region.logical_id())?;
    let (header, scope) = crate::descriptor::metadata(state, value.local_frame.logical_id())?;
    let mut expected = region.scope.clone();
    expected.push(vec![
        AtomicSort::Scalar(Field::Complex),
        AtomicSort::Scalar(Field::Real),
    ]);
    if value.piece != 0 || header != region.header || scope != &expected {
        return Err(invalid());
    }
    let clauses = predicate(state, value.local_frame.logical_id(), &PredicateKind::And)?;
    let [imaginary, real] = clauses else {
        return Err(invalid());
    };
    let imaginary = predicate(state, *imaginary, &PredicateKind::Equal)?;
    let real = predicate(state, *real, &PredicateKind::Equal)?;
    let [component, zero] = imaginary else {
        return Err(invalid());
    };
    let query = component_query(state, *component, &Operator::ImagPart)?;
    if !matches!(&state.expression(*zero)?.operator, Operator::Rational(value) if value.is_zero()) {
        return Err(invalid());
    }
    let [component, local] = real else {
        return Err(invalid());
    };
    if component_query(state, *component, &Operator::RealPart)? != query
        || state.expression(*local)?.operator != (Operator::Bound { depth: 0, slot: 1 })
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
) -> Result<&'a [LogicalId], Fault> {
    match state.node(id)? {
        Vertex::Condition(value) if value.kind == *expected => Ok(&value.arguments),
        Vertex::Expression(value) if value.operator == Operator::Predicate(expected.clone()) => {
            Ok(&value.operands)
        }
        _ => Err(invalid()),
    }
}

/// Require a component of the exact local Complex query slot.
fn component_query(state: &State, id: LogicalId, expected: &Operator) -> Result<LogicalId, Fault> {
    let term = state.expression(id)?;
    let [query] = term.operands.as_slice() else {
        return Err(invalid());
    };
    if term.operator != *expected
        || state.expression(*query)?.operator != (Operator::Bound { depth: 0, slot: 0 })
    {
        return Err(invalid());
    }
    Ok(*query)
}
