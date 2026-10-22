// SPDX-License-Identifier: AGPL-3.0-only
//! Native associations retain original operands and exact placement instances.

use super::admission::{invalid, object};
use super::{FamilyArgumentRole, PlacementUse};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{CaseArm, CaseArmDraft};
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, State, Vertex};
use crate::model::{AtomicSort, Sort};
use crate::solving::SolveDraft;

/// Validate an ordinary or local family arm and preserve every placement association.
pub(crate) fn arm(
    cas: &Cas,
    state: &State,
    draft: &CaseArmDraft,
    ids: (LogicalId, LogicalId, LogicalId),
    locals: &[LogicalId],
    budget: &mut Budget,
) -> Result<CaseArm, Fault> {
    let (header, region, value) = ids;
    let (actual, scope) = crate::descriptor::metadata(state, value)?;
    let region_data = state.region(region)?;
    if scope != &draft.scope
        || region_data.scope != draft.scope
        || !crate::header::embeds(state, actual, header, budget)?
        || !crate::header::embeds(state, region_data.header, header, budget)?
    {
        return Err(invalid());
    }
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(draft.placements.len())
        .map_err(|_| Fault::allocation(super::admission::AT))?;
    for pair in &draft.placements {
        placements.push(PlacementUse {
            operand: cas.resolve(state, &pair.operand, locals)?,
            piece: cas.resolve(state, &pair.piece, locals)?,
        });
    }
    let sort = match state.node(value)? {
        Vertex::Expression(term)
            if matches!(term.sort, Sort::Value(AtomicSort::Scalar(_))) && placements.is_empty() =>
        {
            term.sort.clone()
        }
        Vertex::Solve(body) => {
            let SolveDraft::FamilyExpression(body) = body.as_ref() else {
                return Err(invalid());
            };
            if body.region.logical_id() != region {
                return Err(invalid());
            }
            check_placements(cas, state, body, &placements, budget)?;
            body.result.clone()
        }
        _ => return Err(invalid()),
    };
    Ok(CaseArm {
        placements,
        header,
        scope: draft.scope.clone(),
        region,
        value,
        sort,
    })
}

/// Check exact original operand and registered placement-piece pairs.
fn check_placements(
    cas: &Cas,
    state: &State,
    body: &super::FamilyExpressionData<crate::graph::Handle>,
    placements: &[PlacementUse<LogicalId>],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let operands = placement_operands(state, body, budget)?;
    if operands.len() != placements.len() {
        return Err(invalid());
    }
    for ((operand, ambient), pair) in operands.iter().zip(placements) {
        if *operand != pair.operand {
            return Err(invalid());
        }
        let handle = cas.handle(pair.piece);
        let SolveDraft::ScalarPiece(piece) = object(state, &handle)? else {
            return Err(invalid());
        };
        let SolveDraft::SpacePlacement(placement) = object(state, &piece.placement)? else {
            return Err(invalid());
        };
        if piece.region != body.region
            || placement.ambient.logical_id() != *ambient
            || operand_ambient(state, pair.operand)? != placement.source.logical_id()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Borrow a scalar or local family result's independently checked own ambient.
fn operand_ambient(state: &State, id: LogicalId) -> Result<LogicalId, Fault> {
    match state.node(id)? {
        Vertex::Expression(value) => value.ambient.ok_or_else(invalid),
        Vertex::Solve(value) => match value.as_ref() {
            SolveDraft::FamilyExpression(value) => Ok(value.ambient.logical_id()),
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

/// Nested local bodies retain each distinct original operand/direct-edge requirement.
pub(crate) fn placement_operands(
    state: &State,
    body: &super::FamilyExpressionData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<Vec<(LogicalId, LogicalId)>, Fault> {
    let mut pending = Vec::new();
    let mut expanded = std::collections::BTreeSet::new();
    let mut associated = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    push_arguments(state, &mut pending, body, 1, budget)?;
    while let Some((role, value, ambient, depth)) = pending.pop() {
        budget.work(
            crate::graph::size(
                expanded
                    .len()
                    .saturating_add(associated.len())
                    .saturating_add(1),
                super::admission::AT,
            )?,
            super::admission::AT,
        )?;
        let required = (value.logical_id(), ambient);
        if operand_ambient(state, required.0)? != ambient && associated.insert(required) {
            result
                .try_reserve(1)
                .map_err(|_| Fault::allocation(super::admission::AT))?;
            result.push(required);
        }
        if role == FamilyArgumentRole::Family && expanded.insert(value.logical_id()) {
            let SolveDraft::FamilyExpression(previous) = object(state, &value)? else {
                return Err(invalid());
            };
            push_arguments(
                state,
                &mut pending,
                previous,
                depth.saturating_add(1),
                budget,
            )?;
        }
    }
    Ok(result)
}

/// Prepay all retained pending arguments before copying their handles.
fn push_arguments(
    state: &State,
    pending: &mut Vec<(FamilyArgumentRole, crate::graph::Handle, LogicalId, u64)>,
    body: &super::FamilyExpressionData<crate::graph::Handle>,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let shape = super::admission::family(state, &body.family)?;
    let ambient = shape.fibre_space.logical_id();
    let count = crate::graph::size(
        pending.len().saturating_add(body.arguments.len()),
        super::admission::AT,
    )?;
    budget.check(BudgetKind::Depth, depth, super::admission::AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(128),
        super::admission::AT,
    )?;
    budget.work(count, super::admission::AT)?;
    pending
        .try_reserve(body.arguments.len())
        .map_err(|_| Fault::allocation(super::admission::AT))?;
    pending.extend(
        body.arguments
            .iter()
            .rev()
            .map(|argument| (argument.role, argument.value.clone(), ambient, depth)),
    );
    Ok(())
}
