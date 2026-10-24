// SPDX-License-Identifier: AGPL-3.0-only
//! Live typed coverage is rechecked independently at the consuming publication boundary.

use super::{Plan, Run, SolveAllocation, SolveInventory, Stage, State};
use crate::{
    Bound, Candidate, OperationError, PrivateAllocationError as Error, PrivateAllocationTicket,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

impl Run {
    /// Match both original source association and every numerical owner before retaining a receipt.
    pub(crate) fn publication<P: Scalar, Q: Scalar>(
        bound: &Bound<'_, P, Q>,
        candidate: &Candidate<Q>,
    ) -> Result<Option<Rc<SolveInventory>>, OperationError> {
        let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
            return Ok(None);
        };
        let payload = candidate.outputs.first().ok_or(Error::IncompleteCoverage)?;
        let plan = payload
            .centers
            .solve_plan()
            .ok_or(Error::UnticketedBuffer)?;
        if plan.execution != bound.execution
            || plan.token != candidate.token
            || plan.precision != Q::PRECISION
            || !Rc::ptr_eq(&plan.contract, contract)
            || plan.sources.len() != bound.dependencies.len()
            || !same_symbols(
                &plan.output,
                bound
                    .reservations
                    .symbols
                    .first()
                    .ok_or(Error::IncompleteCoverage)?,
            )
        {
            return Err(Error::ForeignInventory.into());
        }
        for ((source, symbols), actual) in plan.sources.iter().zip(&bound.dependencies) {
            if !source.same_source(&crate::Dependency::from_snapshot(actual))
                || !same_symbols(symbols, &actual.symbols)
            {
                return Err(Error::ForeignInventory.into());
            }
        }
        validate(plan, payload)?;
        let mut allocations = crate::binding::buffer(plan.specifications.len())?;
        for (slot, entry) in plan.specifications.iter().enumerate() {
            if entry.state.get() != State::Planned {
                allocations.push(SolveAllocation {
                    stage: entry.stage,
                    iteration: entry.iteration,
                    extent: entry.extent,
                    ticket: PrivateAllocationTicket::new(plan.execution, plan.token, slot),
                    logical: entry
                        .extent
                        .entries()
                        .checked_mul(plan.width)
                        .ok_or(OperationError::ResourceLimit)?,
                    bytes: entry.bytes.get(),
                });
            }
        }
        let receipt_bytes = allocations
            .capacity()
            .checked_mul(size_of::<SolveAllocation>())
            .ok_or(OperationError::ResourceLimit)?;
        let reserved = plan
            .specifications
            .capacity()
            .checked_mul(size_of::<SolveAllocation>())
            .ok_or(OperationError::ResourceLimit)?;
        let bytes = plan
            .metadata
            .checked_sub(reserved)
            .and_then(|bytes| bytes.checked_add(receipt_bytes))
            .ok_or(OperationError::ResourceLimit)?;
        if bytes > plan.metadata {
            return Err(OperationError::ResourceLimit);
        }
        let candidate_bytes = candidate
            .outputs
            .capacity()
            .checked_mul(size_of::<crate::publication::Payload<Q>>())
            .ok_or(OperationError::ResourceLimit)?;
        Ok(Some(Rc::new(SolveInventory {
            plan: Rc::clone(plan),
            allocations,
            peak: plan.peak.get(),
            bytes,
            candidate: candidate_bytes,
        })))
    }
}
/// Earlier attempts must be complete; only an untouched finite suffix may remain unissued.
fn validate<P: Scalar>(
    plan: &Rc<Plan>,
    payload: &crate::publication::Payload<P>,
) -> Result<(), OperationError> {
    if plan.completed.get() == 0 {
        return Err(Error::IncompleteCoverage.into());
    }
    for (slot, entry) in plan.specifications.iter().enumerate() {
        if let Some(iteration) = entry.iteration {
            let expected = if iteration < plan.completed.get() {
                State::Released(entry.extent.entries())
            } else {
                State::Planned
            };
            if entry.state.get() != expected {
                return Err(Error::IncompleteCoverage.into());
            }
        } else {
            let values = match entry.stage {
                Stage::OutputCenters => Some(&payload.centers),
                Stage::OutputTotals => payload.bounds.as_ref(),
                Stage::OutputErrors => payload.evaluation.as_ref(),
                _ => None,
            }
            .ok_or(Error::IncompleteCoverage)?;
            if entry.state.get() != State::Candidate
                || values.len() != entry.extent.entries()
                || !values.matches_solve(plan, slot)
            {
                return Err(Error::UnticketedBuffer.into());
            }
        }
    }
    Ok(())
}

/// Equal shapes or values do not replace original value and companion declarations.
fn same_symbols(left: &nrz_bindings::BindingSymbols, right: &nrz_bindings::BindingSymbols) -> bool {
    left.value() == right.value() && left.bound() == right.bound() && left.error() == right.error()
}
