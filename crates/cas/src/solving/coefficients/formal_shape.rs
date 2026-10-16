// SPDX-License-Identifier: AGPL-3.0-only
//! Exact, charged scope erasure compares payloads and ordered semantic dependencies.

use super::storage::{invalid, reserve, vector};
use crate::budget::Budget;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Draft, ExpressionAttachment, Handle, LogicalId, Reference, State, Vertex};
use crate::model::{AtomicSort, Field, Scope};
use crate::solving::SolveDraft;

/// Comparisons and worklist operations use ordinary cumulative work units.
pub(crate) const AT: Location = Location::new("formal root scope erasure");

/// Only one complete nonempty complex frame may be introduced or removed.
pub(crate) fn frame(scope: &Scope) -> Result<(), Fault> {
    let [slots] = scope.as_slice() else {
        return Err(invalid());
    };
    if slots.is_empty()
        || slots
            .iter()
            .any(|sort| *sort != AtomicSort::Scalar(Field::Complex))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Scope-free dependencies remain unchanged; the eliminated frame has an empty target.
fn scopes(source: &Scope, target: &[Vec<AtomicSort>], frame: &Scope) -> Result<(), Fault> {
    if !target.is_empty() || (!source.is_empty() && source != frame) {
        return Err(invalid());
    }
    Ok(())
}

/// Each operand comparison is prepaid, including the full metadata payload.
fn charge(state: &State, id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    let (_, key) = crate::graph::vertex_key(state.node(id)?, state, budget)?;
    budget.work(crate::graph::size(key.len(), AT)?.max(1), AT)
}

/// Append an ordered dependency family after checking its complete extent.
fn rows(
    pending: &mut Vec<(LogicalId, LogicalId)>,
    left: &[LogicalId],
    right: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(crate::graph::size(left.len(), AT)?.max(1), AT)?;
    if left.len() != right.len() {
        return Err(invalid());
    }
    reserve(pending, left.len(), budget)?;
    pending.extend(left.iter().copied().zip(right.iter().copied()));
    Ok(())
}

/// Attached coefficients preserve their value and explicit evaluation map.
fn attachments(
    left: Option<&ExpressionAttachment>,
    right: Option<&ExpressionAttachment>,
    pending: &mut Vec<(LogicalId, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match (left, right) {
        (None, None) => Ok(()),
        (
            Some(ExpressionAttachment::SelectedRoot(a)),
            Some(ExpressionAttachment::SelectedRoot(b)),
        ) if a == b => Ok(()),
        (
            Some(ExpressionAttachment::ExactCoefficient { value: a, map: b }),
            Some(ExpressionAttachment::ExactCoefficient { value: c, map: d }),
        ) if a == c => rows(pending, &[*b], &[*d], budget),
        _ => Err(invalid()),
    }
}

/// Scalar slots, operators, sorts and non-scope metadata cannot change during erasure.
fn expressions(
    left: &crate::graph::Expression,
    right: &crate::graph::Expression,
    scope: &Scope,
    pending: &mut Vec<(LogicalId, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    scopes(&left.scope, &right.scope, scope)?;
    if (
        &left.operator,
        &left.sort,
        left.header,
        left.layer,
        left.space,
        &left.binding,
    ) != (
        &right.operator,
        &right.sort,
        right.header,
        right.layer,
        right.space,
        &right.binding,
    ) || left.binding.is_some()
        || left.domain.is_some()
        || right.domain.is_some()
        || !left.arms.is_empty()
        || !right.arms.is_empty()
    {
        return Err(invalid());
    }
    attachments(
        left.attachment.as_ref(),
        right.attachment.as_ref(),
        pending,
        budget,
    )?;
    rows(pending, &left.operands, &right.operands, budget)
}

/// Region constructors retain exact guards and do not gain algebraic simplification.
fn regions(
    left: &crate::domain_model::Region,
    right: &crate::domain_model::Region,
    scope: &Scope,
    pending: &mut Vec<(LogicalId, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use crate::domain_model::RegionExpr;
    scopes(&left.scope, &right.scope, scope)?;
    if (left.header, &left.ambient) != (right.header, &right.ambient) {
        return Err(invalid());
    }
    match (&left.expression, &right.expression) {
        (RegionExpr::Ambient, RegionExpr::Ambient) | (RegionExpr::Empty, RegionExpr::Empty) => {
            Ok(())
        }
        (
            RegionExpr::GuardTrue {
                base: a,
                predicate: b,
            },
            RegionExpr::GuardTrue {
                base: c,
                predicate: d,
            },
        )
        | (
            RegionExpr::GuardFalse {
                base: a,
                predicate: b,
            },
            RegionExpr::GuardFalse {
                base: c,
                predicate: d,
            },
        )
        | (
            RegionExpr::Difference {
                base: a,
                excluded: b,
            },
            RegionExpr::Difference {
                base: c,
                excluded: d,
            },
        ) => rows(pending, &[*a, *b], &[*c, *d], budget),
        (RegionExpr::Meet(a), RegionExpr::Meet(b))
        | (RegionExpr::Union(a), RegionExpr::Union(b)) => rows(pending, a, b, budget),
        _ => Err(invalid()),
    }
}

/// A map retains its source, target, mode and complete denominator inventory exactly.
fn maps(
    left: &crate::solving::CoefficientMapData<Handle>,
    right: &crate::solving::CoefficientMapData<Handle>,
    pending: &mut Vec<(LogicalId, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if (
        &left.source,
        &left.target,
        left.mode,
        &left.denominators,
        left.images.len(),
    ) != (
        &right.source,
        &right.target,
        right.mode,
        &right.denominators,
        right.images.len(),
    ) {
        return Err(invalid());
    }
    reserve(
        pending,
        left.images.len().checked_add(1).ok_or_else(invalid)?,
        budget,
    )?;
    pending.push((left.region.logical_id(), right.region.logical_id()));
    pending.extend(
        left.images
            .iter()
            .zip(&right.images)
            .map(|(a, b)| (a.logical_id(), b.logical_id())),
    );
    Ok(())
}

/// Only the enumerated syntax kinds can differ; formal polynomial slots retain graph identity.
fn node(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    scope: &Scope,
    pending: &mut Vec<(LogicalId, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    charge(state, left, budget)?;
    charge(state, right, budget)?;
    match (state.node(left)?, state.node(right)?) {
        (Vertex::Expression(a), Vertex::Expression(b)) => expressions(a, b, scope, pending, budget),
        (Vertex::Region(a), Vertex::Region(b)) => regions(a, b, scope, pending, budget),
        (Vertex::Condition(a), Vertex::Condition(b)) => {
            scopes(&a.scope, &b.scope, scope)?;
            if (&a.kind, a.header) != (&b.kind, b.header) {
                return Err(invalid());
            }
            rows(pending, &a.arguments, &b.arguments, budget)
        }
        (Vertex::Solve(a), Vertex::Solve(b)) => match (a.as_ref(), b.as_ref()) {
            (SolveDraft::CoefficientMap(a), SolveDraft::CoefficientMap(b)) => {
                maps(a, b, pending, budget)
            }
            _ if left == right => Ok(()),
            _ => Err(invalid()),
        },
        _ if left == right => Ok(()),
        _ => Err(invalid()),
    }
}

/// Charge every visited pair and comparison; a notification or equal printed name is no evidence.
pub(crate) fn paired(
    state: &State,
    source: LogicalId,
    target: LogicalId,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    frame(scope)?;
    let mut pending = vector(1, budget)?;
    pending.push((source, target));
    let mut seen = vector(0, budget)?;
    while let Some(pair) = pending.pop() {
        budget.work(crate::graph::size(seen.len(), AT)?.max(1), AT)?;
        if seen.contains(&pair) {
            continue;
        }
        reserve(&mut seen, 1, budget)?;
        seen.push(pair);
        node(state, pair.0, pair.1, scope, &mut pending, budget)?;
    }
    Ok(())
}

/// A changed admission scope is allowed only for the exact lifted parent region.
pub(crate) fn introduction(
    state: &State,
    parent: LogicalId,
    region: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let base = state.region(parent)?;
    let local = state.region(region)?;
    if base.scope == local.scope {
        return Ok(false);
    }
    if !base.scope.is_empty() {
        return Err(invalid());
    }
    frame(&local.scope)?;
    paired(state, region, parent, &local.scope, budget)?;
    Ok(true)
}

/// Dependencies rebuilt during condition erasure are limited to the supported syntax vocabulary.
fn children(
    cas: &Cas,
    id: LogicalId,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<Option<Vec<LogicalId>>, Fault> {
    charge(&cas.state, id, budget)?;
    let mut result = vector(0, budget)?;
    match cas.state.node(id)? {
        Vertex::Expression(value) => {
            scopes(&value.scope, &[], scope)?;
            if value.scope.is_empty() {
                return Ok(None);
            }
            if value.binding.is_some() || value.domain.is_some() || !value.arms.is_empty() {
                return Err(invalid());
            }
            reserve(
                &mut result,
                value.operands.len().checked_add(1).ok_or_else(invalid)?,
                budget,
            )?;
            result.extend_from_slice(&value.operands);
            match value.attachment {
                Some(ExpressionAttachment::ExactCoefficient { map, .. }) => result.push(map),
                Some(ExpressionAttachment::SelectedRoot(_)) => return Err(invalid()),
                None => {}
            }
        }
        Vertex::Condition(value) => {
            scopes(&value.scope, &[], scope)?;
            if value.scope.is_empty() {
                return Ok(None);
            }
            result = super::storage::copied(&value.arguments, budget)?;
        }
        Vertex::Region(value) => {
            scopes(&value.scope, &[], scope)?;
            if value.scope.is_empty() {
                return Ok(None);
            }
            let children = crate::descriptor::children(&cas.state, id, budget)?;
            reserve(&mut result, children.len(), budget)?;
            result.extend(children.into_iter().map(|(_, child)| child));
        }
        Vertex::Solve(value) => {
            let SolveDraft::CoefficientMap(map) = value.as_ref() else {
                return Ok(None);
            };
            reserve(
                &mut result,
                map.images.len().checked_add(1).ok_or_else(invalid)?,
                budget,
            )?;
            result.push(map.region.logical_id());
            result.extend(map.images.iter().map(Handle::logical_id));
        }
        _ => return Ok(None),
    }
    Ok(Some(result))
}

/// Reconstruct only scope metadata and already checked child references, never an operator value.
fn draft(
    cas: &Cas,
    id: LogicalId,
    known: &std::collections::BTreeMap<LogicalId, Handle>,
) -> Result<Draft, Fault> {
    let mut resolve = |child| {
        known
            .get(&child)
            .cloned()
            .map(Reference::Existing)
            .ok_or_else(invalid)
    };
    match cas.state.node(id)? {
        Vertex::Expression(value) => {
            if let Some(ExpressionAttachment::ExactCoefficient {
                value: coefficient,
                map,
            }) = value.attachment
            {
                return Ok(Draft::ExactCoefficient {
                    header: Reference::Existing(cas.handle(value.header)),
                    value: Reference::Existing(cas.handle(coefficient)),
                    map: resolve(map)?,
                });
            }
            crate::substitution_draft::rebuild(
                cas,
                id,
                Vec::new(),
                Some(value.operator.clone()),
                &mut resolve,
            )
        }
        Vertex::Region(_) | Vertex::Condition(_) => {
            crate::substitution_draft::rebuild(cas, id, Vec::new(), None, &mut resolve)
        }
        Vertex::Solve(value) => {
            let SolveDraft::CoefficientMap(map) = value.as_ref() else {
                return Err(invalid());
            };
            Ok(Draft::Solve(SolveDraft::CoefficientMap(
                crate::solving::CoefficientMapData {
                    source: Reference::Existing(map.source.clone()),
                    target: match &map.target {
                        crate::solving::MapTarget::CoefficientDomain { domain } => {
                            crate::solving::MapTarget::CoefficientDomain {
                                domain: Reference::Existing(domain.clone()),
                            }
                        }
                        crate::solving::MapTarget::ScalarAmbient { header, field } => {
                            crate::solving::MapTarget::ScalarAmbient {
                                header: Reference::Existing(header.clone()),
                                field: field.clone(),
                            }
                        }
                    },
                    mode: map.mode,
                    region: resolve(map.region.logical_id())?,
                    images: map
                        .images
                        .iter()
                        .map(|value| resolve(value.logical_id()))
                        .collect::<Result<_, _>>()?,
                    denominators: map
                        .denominators
                        .iter()
                        .cloned()
                        .map(Reference::Existing)
                        .collect(),
                },
            )))
        }
        _ => Err(invalid()),
    }
}

/// Rebuild retained obligations inside the existing atomic proof transaction, preserving their kind.
pub(crate) fn erase(
    cas: &mut Cas,
    root: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    super::independent::object(cas, root, scope, budget)?;
    let mut pending = vector(1, budget)?;
    pending.push((cas.checked(root)?, false));
    let mut known = std::collections::BTreeMap::new();
    while let Some((id, expanded)) = pending.pop() {
        budget.work(crate::graph::size(known.len(), AT)?.max(1), AT)?;
        if known.contains_key(&id) {
            continue;
        }
        let Some(dependencies) = children(cas, id, scope, budget)? else {
            known.insert(id, cas.handle(id));
            continue;
        };
        if expanded {
            let value = draft(cas, id, &known)?;
            let handle = cas
                .admit(std::slice::from_ref(&value), budget)?
                .into_iter()
                .next()
                .ok_or_else(invalid)?;
            paired(&cas.state, id, handle.logical_id(), scope, budget)?;
            known.insert(id, handle);
        } else {
            reserve(
                &mut pending,
                dependencies.len().checked_add(1).ok_or_else(invalid)?,
                budget,
            )?;
            pending.push((id, true));
            pending.extend(dependencies.into_iter().map(|child| (child, false)));
        }
    }
    known.remove(&root.logical_id()).ok_or_else(invalid)
}
