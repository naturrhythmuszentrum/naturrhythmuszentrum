// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed local proof identity retains the complete canonical map key and all original guards.

use super::{AT, NativeRecipe, OperationError};
use crate::{CheckedOperation, RegionContract};
use nrz_bindings::Budget;
use nrz_scalar::LiteralLimits;
use std::rc::Rc;

/// The closed recipe's conservative numerical phase charge includes all coefficient imports.
pub(super) fn work(operation: &CheckedOperation) -> Result<u64, OperationError> {
    let NativeRecipe::Reciprocal { kernel, .. } = &operation.recipe else {
        return Err(crate::HeapError::MissingBinding.into());
    };
    u64::try_from(kernel.work).map_err(|_| OperationError::ResourceLimit)
}

/// Immutable proof handles authenticate the canonical key without rescanning its retained bytes.
pub(super) fn same_operation(
    left: &CheckedOperation,
    right: &CheckedOperation,
    budget: &mut Budget,
) -> Result<bool, OperationError> {
    let (Some(first), Some(second)) = (left.reciprocal_map(), right.reciprocal_map()) else {
        return Ok(false);
    };
    if left.revision() != right.revision()
        || literals(left)? != literals(right)?
        || first.witness().proof() != second.witness().proof()
        || first.original().expression() != second.original().expression()
        || first.image().expression() != second.image().expression()
    {
        return Ok(false);
    }
    Ok(contracts(&left.inputs, &right.inputs, budget)?
        && contracts(&left.outputs, &right.outputs, budget)?)
}

/// Import policy is an explicit part of the arithmetic model, even when decoded bits agree.
fn literals(operation: &CheckedOperation) -> Result<LiteralLimits, OperationError> {
    let NativeRecipe::Reciprocal { kernel, .. } = &operation.recipe else {
        return Err(crate::KernelError::Contract.into());
    };
    let crate::kernels::contract::Recipe::PositiveReciprocal(schedule) = &kernel.recipe else {
        return Err(crate::KernelError::Contract.into());
    };
    Ok(schedule.literals)
}

/// Shared frozen contracts compare in constant time; independent snapshots compare every entry.
fn contracts(
    left: &Rc<[RegionContract]>,
    right: &Rc<[RegionContract]>,
    budget: &mut Budget,
) -> Result<bool, OperationError> {
    if Rc::ptr_eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right.iter()) {
        let count = left
            .shape()
            .extent()
            .entries()
            .checked_add(1)
            .ok_or(OperationError::ResourceLimit)?;
        budget.work(
            u64::try_from(count).map_err(|_| OperationError::ResourceLimit)?,
            AT,
        )?;
        let first = left.shape();
        let second = right.shape();
        if left.descriptor() != right.descriptor()
            || first.declarations().id() != second.declarations().id()
            || first.domain() != second.domain()
            || first.codomain() != second.codomain()
            || first.extent() != second.extent()
            || !entries(first.entries(), second.entries())
            || first.structure_scope() != second.structure_scope()
            || first.recipe() != second.recipe()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Original syntax and its frozen semantic snapshot determine each sealed entry interpretation.
fn entries(left: &[crate::EntryContract], right: &[crate::EntryContract]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            let evidence = match (left.checked_expression(), right.checked_expression()) {
                (Some(first), Some(second)) => {
                    first.snapshot_id() == second.snapshot_id()
                        && first.original().syntax == second.original().syntax
                        && first.original().root == second.original().root
                }
                (None, None) => true,
                _ => false,
            };
            evidence
                && left.pairing() == right.pairing()
                && left.scalar_descriptor() == right.scalar_descriptor()
                && left.conversion() == right.conversion()
                && left.is_zero() == right.is_zero()
                && left.original() == right.original()
        })
}
