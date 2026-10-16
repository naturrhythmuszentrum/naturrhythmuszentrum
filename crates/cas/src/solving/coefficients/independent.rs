// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded semantic dependency checks for existential formal-root elimination.

use super::storage::{AT, invalid, reserve, vector};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Vertex};
use crate::model::{Operator, Scope};
use crate::proof_model::ScopedConditions;
use petgraph::visit::EdgeRef;

/// Inspect semantic dependencies, including attached maps, guards and nested binder references.
pub(crate) fn object(
    cas: &Cas,
    handle: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let index = *cas
        .state
        .indices
        .get(&cas.checked(handle)?)
        .ok_or_else(invalid)?;
    let mut pending = vector(1, budget)?;
    pending.push(index);
    let mut seen = vector(0, budget)?;
    while let Some(current) = pending.pop() {
        budget.work(crate::graph::size(seen.len(), AT)?.max(1), AT)?;
        if seen.contains(&current) {
            continue;
        }
        reserve(&mut seen, 1, budget)?;
        seen.push(current);
        let vertex = cas.state.graph.node_weight(current).ok_or_else(invalid)?;
        if let Vertex::Expression(expression) = vertex
            && let Operator::Bound { depth, .. } = expression.operator
            && expression.scope.starts_with(scope)
            && expression
                .scope
                .len()
                .checked_sub(usize::try_from(depth).map_err(|_| invalid())?)
                .and_then(|value| value.checked_sub(1))
                == scope.len().checked_sub(1)
        {
            return Err(invalid());
        }
        for edge in cas.state.graph.edges(current) {
            budget.work(1, AT)?;
            reserve(&mut pending, 1, budget)?;
            pending.push(edge.target());
        }
    }
    Ok(())
}

/// Only the local Vieta frame may depend on these roots; enclosing assumptions remain independent.
pub(crate) fn surroundings(
    cas: &Cas,
    context: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let local = cas.state.context(cas.checked(context)?)?;
    let mut parent = local.parent;
    while let Some(id) = parent {
        budget.work(1, AT)?;
        let frame = cas.state.context(id)?;
        for (_, predicate) in frame.frame.iter().chain(&frame.assumptions) {
            object(cas, &cas.handle(*predicate), scope, budget)?;
        }
        object(cas, &cas.handle(frame.admissible), scope, budget)?;
        parent = frame.parent;
    }
    Ok(())
}

/// Conditions retain their meaning and provenance; none may constrain the existential witness.
pub(crate) fn conditions(
    cas: &Cas,
    inherited: &ScopedConditions,
    context: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for hypothesis in &inherited.hypotheses {
        if hypothesis.context != *context && hypothesis.slot != 0 {
            object(
                cas,
                &crate::context_check::predicate(cas, hypothesis)?,
                scope,
                budget,
            )?;
        }
    }
    for condition in inherited.requirements.iter().chain(&inherited.residuals) {
        object(cas, &condition.predicate, scope, budget)?;
        object(cas, &condition.region, scope, budget)?;
    }
    Ok(())
}
