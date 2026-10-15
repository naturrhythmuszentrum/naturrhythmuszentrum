// SPDX-License-Identifier: AGPL-3.0-only
//! Exact selection predicates used by root existence and component transport.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;
use crate::model::{Operator, PredicateKind};
use crate::solving::RootSelectionData;
use crate::solving::coefficients::storage::{AT, invalid};

/// Recognize a universally true one-slot equality with a manifestly defined argument.
pub(crate) fn unconstrained(
    cas: &Cas,
    selection: &RootSelectionData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let crate::graph::Vertex::Condition(predicate) =
        cas.state.node(cas.checked(&selection.predicate)?)?
    else {
        return Err(invalid());
    };
    if predicate.kind != PredicateKind::Equal {
        return Err(invalid());
    }
    let [left, right] = predicate.arguments.as_slice() else {
        return Err(invalid());
    };
    if left != right {
        return Err(invalid());
    }
    let expression = cas.state.expression(*left)?;
    if !matches!(
        expression.operator,
        Operator::Bound { depth: 0, slot: 0 } | Operator::Rational(_)
    ) {
        return Err(invalid());
    }
    Ok(())
}
