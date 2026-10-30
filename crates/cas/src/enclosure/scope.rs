// SPDX-License-Identifier: AGPL-3.0-only
//! Scope keys retain full portable source identities, never logical serials alone.

use super::{AT, EnclosureGoal, EnclosureRequest, RefinementPolicy};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};

/// Refuse unsupported metadata and impossible schedules before charging Work.
pub(super) fn preflight(
    cas: &Cas,
    request: &EnclosureRequest,
    policy: RefinementPolicy,
    budget: &Budget,
) -> Result<Option<u32>, Fault> {
    let target = cas.state.expression(cas.checked(&request.target)?)?;
    let context = cas.state.context(cas.checked(&request.context)?)?;
    if target.header != context.header_id || !target.scope.is_empty() {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    if !target.supported_enclosure || policy.version != 1 {
        return Ok(None);
    }
    let Some(grid) = policy.precision.checked_add(8) else {
        return Ok(None);
    };
    if policy.rounds == 0 || grid > policy.grid_ceiling {
        return Ok(None);
    }
    budget.check(BudgetKind::IntegerExponent, u64::from(grid), AT)?;
    budget.check(BudgetKind::ComponentBits, u64::from(grid) + 1, AT)?;
    budget.check(BudgetKind::IntermediateBits, u64::from(grid) + 1, AT)?;
    if target.enclosure_roots {
        budget.check(BudgetKind::IntegerExponent, u64::from(grid) * 2, AT)?;
        budget.check(BudgetKind::IntermediateBits, u64::from(grid) * 2 + 1, AT)?;
    }
    budget.check(BudgetKind::Depth, target.depth, AT)?;
    budget.check(BudgetKind::ProofNodes, target.unfolded, AT)?;
    budget.check(
        BudgetKind::MatrixEntries,
        size(request.coordinates.len(), AT)?,
        AT,
    )?;
    if policy.checking_work > budget.limits().work.saturating_sub(budget.usage().work) {
        return Err(Fault::exhausted(
            BudgetKind::Work,
            budget.limits().work,
            policy.checking_work.saturating_add(budget.usage().work),
            AT,
        ));
    }
    Ok(Some(grid))
}

/// Encode the complete portable original request after validating its box.
pub(super) fn key(
    cas: &Cas,
    request: &EnclosureRequest,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    validate(cas, request, budget)?;
    let count = request
        .coordinates
        .len()
        .checked_add(2)
        .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(count, AT)?.saturating_mul(32),
        AT,
    )?;
    let mut blobs = Vec::new();
    blobs
        .try_reserve(count)
        .map_err(|_| Fault::allocation(AT))?;
    for handle in [&request.target, &request.context]
        .into_iter()
        .chain(request.coordinates.iter().map(|v| &v.coordinate))
    {
        let bytes = crate::wire_build::syntax_key(cas, cas.checked(handle)?, budget)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            footprint_blobs(&blobs).saturating_add(size(bytes.len(), AT)?),
            AT,
        )?;
        blobs.push(bytes);
    }
    let live = footprint_blobs(&blobs).saturating_add(size(count, AT)?.saturating_mul(32));
    budget.with_retained_payload(live, AT, |budget| encode(request, blobs, budget))
}

/// Frame the scope while retaining and accounting for all source-key buffers.
fn encode(
    request: &EnclosureRequest,
    blobs: Vec<Vec<u8>>,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(budget, AT);
    out.raw(b"NRZENC01")?;
    for bytes in blobs {
        out.count(bytes.len())?;
        out.raw(&bytes)?;
    }
    out.count(request.coordinates.len())?;
    for coordinate in &request.coordinates {
        out.number(coordinate.interval.lower())?;
        out.number(coordinate.interval.upper())?;
    }
    match &request.goal {
        EnclosureGoal::Width => out.byte(0)?,
        EnclosureGoal::AtMost(value) => {
            out.byte(1)?;
            out.number(value)?;
        }
        EnclosureGoal::LessThan(value) => {
            out.byte(2)?;
            out.number(value)?;
        }
        EnclosureGoal::Equal(value) => {
            out.byte(3)?;
            out.number(value)?;
        }
    }
    Ok(out.finish())
}

/// Account for the syntax buffers retained while assembling the scope.
fn footprint_blobs(blobs: &[Vec<u8>]) -> u64 {
    blobs.iter().fold(0_u64, |sum, v| {
        sum.saturating_add(u64::try_from(v.len()).unwrap_or(u64::MAX))
    })
}

/// Check coordinate ownership, scalar sorts and uniqueness.
fn validate(cas: &Cas, request: &EnclosureRequest, budget: &mut Budget) -> Result<(), Fault> {
    let target = cas.state.expression(cas.checked(&request.target)?)?;
    for (index, coordinate) in request.coordinates.iter().enumerate() {
        budget.work(
            u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1),
            AT,
        )?;
        let expression = cas.state.expression(cas.checked(&coordinate.coordinate)?)?;
        if !matches!(expression.operator, Operator::Free(_))
            || expression.header != target.header
            || !expression.scope.is_empty()
            || !scalar(&expression.sort)
            || request.coordinates[..index]
                .iter()
                .any(|previous| previous.coordinate == coordinate.coordinate)
        {
            return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
        }
        coordinate.interval.lower().check(budget, AT)?;
        coordinate.interval.upper().check(budget, AT)?;
    }
    if !scalar(&target.sort) {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    Ok(())
}

/// Recognize scalar sorts supported by rational endpoint arithmetic.
const fn scalar(sort: &Sort) -> bool {
    matches!(
        sort,
        Sort::Value(AtomicSort::Scalar(Field::Real | Field::Rational) | AtomicSort::Integer)
    )
}

/// Iterative postorder preserves every original occurrence, including undefined hidden children.
pub(super) fn schedule(
    cas: &Cas,
    request: &EnclosureRequest,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let root = cas.checked(&request.target)?;
    let count = cas.state.expression(root)?.unfolded;
    budget.check(BudgetKind::ProofNodes, count, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(128),
        AT,
    )?;
    budget.work(count.saturating_mul(2), AT)?;
    let capacity =
        usize::try_from(count).map_err(|_| Fault::capacity(BudgetKind::ProofNodes, AT))?;
    let mut pending = Vec::new();
    let mut result = Vec::new();
    pending
        .try_reserve(capacity)
        .map_err(|_| Fault::allocation(AT))?;
    result
        .try_reserve(capacity)
        .map_err(|_| Fault::allocation(AT))?;
    pending.push((root, false));
    while let Some((id, visited)) = pending.pop() {
        if visited {
            result.push(id);
            continue;
        }
        let value = cas.state.expression(id)?;
        if !value.supported_enclosure || !value.scope.is_empty() || !scalar(&value.sort) {
            return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
        }
        pending.push((id, true));
        for child in value.operands.iter().rev() {
            pending.push((*child, false));
        }
    }
    Ok(result)
}
