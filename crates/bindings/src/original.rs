// SPDX-License-Identifier: AGPL-3.0-only
//! Located views into CAS-owned original syntax, without another expression tree.

use nrz_cas::limits::Budget;
use nrz_cas::source::{OriginalForm, SyntaxNode, SyntaxRole, Visit, WalkEvent};
use nrz_cas::syntax::{AtomicSort, Field, Operator, Sort, Span};
use nrz_cas::{Cas, Fault, Handle, InvalidCode, Location, Reference};
use std::collections::BTreeMap;

/// One inspected original operation and its immediate argument byte intervals.
pub(crate) struct Source {
    /// Shared CAS syntax identity, or a staged free symbol for a relationship.
    pub(super) expression: Reference,
    /// Actual immutable declaration header.
    pub(super) header: Handle,
    /// Written operator, inspected before any coordinate normalization.
    pub(super) operator: Operator,
    /// Original operation's byte interval.
    pub(super) span: Option<Span>,
    /// Original operand spans, in syntax order.
    pub(super) arguments: Vec<Option<Span>>,
}

impl Source {
    /// Inspect every dependency role instead of treating arbitrary syntax as scalar arithmetic.
    pub(super) fn inspect(
        cas: &Cas,
        visit: &Visit,
        spans: &BTreeMap<Vec<u32>, Option<Span>>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let descriptor = cas.describe_syntax(&visit.expression, budget)?;
        let SyntaxNode::Expression(syntax) = descriptor.node else {
            return Err(unsupported());
        };
        if syntax.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
            || !descriptor.scope.is_empty()
            || descriptor.binding.is_some()
        {
            return Err(unsupported());
        }
        let mut arguments = Vec::new();
        for (index, dependency) in descriptor.dependencies.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| unsupported())?;
            if dependency.role != SyntaxRole::Operand(index) {
                return Err(unsupported());
            }
            let mut path = visit.path.clone();
            path.push(index);
            arguments.push(spans.get(&path).copied().flatten());
        }
        Ok(Self {
            expression: Reference::Existing(visit.expression.clone()),
            header: descriptor.header,
            operator: syntax.operator,
            span: visit.source.as_ref().and_then(|source| source.span),
            arguments,
        })
    }

    /// Inspect a declaration body's root while retaining its original source interval.
    pub(super) fn root(
        cas: &Cas,
        original: &OriginalForm,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let walk = cas
            .walk_original(&original.root, budget)
            .map_err(|error| error.fault)?;
        let Some(WalkEvent::Visit(visit)) = walk.events.last() else {
            return Err(unsupported());
        };
        if visit.expression != original.syntax {
            return Err(unsupported());
        }
        Self::inspect(cas, visit, &spans(&walk.events), budget)
    }
}

/// Keep source intervals indexed by original occurrence paths, including repeated syntax.
pub(crate) fn spans(events: &[WalkEvent]) -> BTreeMap<Vec<u32>, Option<Span>> {
    events
        .iter()
        .filter_map(|event| match event {
            WalkEvent::Visit(visit) => Some((
                visit.path.clone(),
                visit.source.as_ref().and_then(|source| source.span),
            )),
            WalkEvent::EnterBinder { .. } => None,
        })
        .collect()
}

/// The physical scalar checker refuses syntax outside its admitted operation fragment.
pub(crate) const fn unsupported() -> Fault {
    Fault::invalid(
        InvalidCode::UnsupportedFragment,
        Location::new("bindings original syntax"),
    )
}
