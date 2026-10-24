// SPDX-License-Identifier: AGPL-3.0-only
//! Exact aligned inventory owners and selection capacities are admitted before numerical allocation.

use super::plan::{Plan, RcLayout};
use crate::{
    OperationError,
    statistics::{
        capture::{InputBinding, Origin},
        store::Lineage,
    },
};
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};

/// Shared original definitions are charged once only when this capture creates their owner.
pub(super) fn bytes(plan: &Plan) -> Result<usize, OperationError> {
    parts(plan)?.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value).ok_or(OperationError::ResourceLimit)
    })
}
/// Plan, receipt header, request table/projections, operands, common order, trace, new lineage, selections.
pub(super) fn parts(plan: &Plan) -> Result<[usize; 8], OperationError> {
    let mut parts = [
        size_of::<RcLayout<Plan>>(),
        size_of::<RcLayout<super::StatisticsInventory>>(),
        product(
            plan.specifications.capacity(),
            size_of::<super::plan::Specification>() + size_of::<super::StatisticsAllocation>(),
        )?,
        product(plan.bindings.capacity(), size_of::<InputBinding>())?,
        product(plan.columns.capacity(), size_of::<usize>())?,
        if plan.operation.is_none() {
            crate::execution::Trace::metadata_bytes()
        } else {
            0
        },
        0,
        0,
    ];
    for input in &plan.bindings {
        let maps = product(input.rows.capacity(), size_of::<usize>())?
            .checked_add(input.dependency.layout().owned_bytes())
            .ok_or(OperationError::ResourceLimit)?;
        parts[7] = parts[7]
            .checked_add(maps)
            .ok_or(OperationError::ResourceLimit)?;
        if matches!(input.origin, Origin::Declared { .. }) {
            parts[6] = parts[6]
                .checked_add(lineage(&input.lineage)?)
                .ok_or(OperationError::ResourceLimit)?;
        }
    }
    Ok(parts)
}
/// Propagated lineage is already shared; only newly captured joint declarations own these definitions.
fn lineage(value: &Lineage) -> Result<usize, OperationError> {
    let capacities = [
        size_of::<RcLayout<Lineage>>(),
        product(value.columns.capacity(), size_of::<usize>())?,
        product(value.roots.capacity(), size_of::<usize>())?,
        product(value.model.sources.capacity(), size_of::<crate::SourceId>())?,
    ];
    capacities.into_iter().try_fold(0_usize, |sum, count| {
        sum.checked_add(count).ok_or(OperationError::ResourceLimit)
    })
}
/// Every capacity product refuses overflow before the numerical allocation phase.
const fn product(count: usize, width: usize) -> Result<usize, OperationError> {
    match count.checked_mul(width) {
        Some(bytes) => Ok(bytes),
        None => Err(OperationError::ResourceLimit),
    }
}
/// Both explicit caller byte ceilings precede the first covered numerical owner.
pub(super) fn check(bytes: usize, budget: &Budget) -> Result<(), OperationError> {
    let bytes = u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?;
    let at = Location::new("statistical allocation inventory");
    budget.check(BudgetKind::StagedPayloadBytes, bytes, at)?;
    budget.check(BudgetKind::OutputBytes, bytes, at)?;
    Ok(())
}
