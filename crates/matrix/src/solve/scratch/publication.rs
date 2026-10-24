// SPDX-License-Identifier: AGPL-3.0-only
//! Live typed coverage is rechecked independently at the consuming publication boundary.

use super::{Plan, Run, SolveAllocation, SolveInventory, Stage, State};
use crate::{Bound, Candidate, OperationError, PrivateAllocationError as Error};
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
        let same_derivative = match (&plan.derivative, &bound.statistics.derivative) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        };
        if !same_derivative {
            return Err(Error::ForeignInventory.into());
        }
        let same_statistics = match (&plan.statistics, &bound.statistics.lineage) {
            (Some(original), Some(_)) => original.matches(&bound.statistics),
            (None, None) => true,
            _ => false,
        };
        if !same_statistics {
            return Err(Error::ForeignInventory.into());
        }
        validate(plan, candidate)?;
        receipt(plan, candidate)
    }
}
/// Snapshot only after all publication copies have been prepared and their owners released.
fn receipt<Q: Scalar>(
    plan: &Rc<Plan>,
    candidate: &Candidate<Q>,
) -> Result<Option<Rc<SolveInventory>>, OperationError> {
    let mut allocations = crate::binding::buffer(plan.specifications.len())?;
    for (slot, entry) in plan.specifications.iter().enumerate() {
        if entry.state.get() != State::Planned {
            entry
                .extent
                .entries()
                .checked_mul(entry.stage.components())
                .and_then(|count| count.checked_mul(plan.width))
                .ok_or(OperationError::ResourceLimit)?;
            allocations.push(SolveAllocation {
                plan: Rc::clone(plan),
                slot,
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
/// Earlier attempts must be complete; only an untouched finite suffix may remain unissued.
fn validate<P: Scalar>(plan: &Rc<Plan>, candidate: &Candidate<P>) -> Result<(), OperationError> {
    if plan.completed.get() == 0 {
        return Err(Error::IncompleteCoverage.into());
    }
    for (slot, entry) in plan.specifications.iter().enumerate() {
        if entry.stage.is_candidate_at(entry.iteration) {
            validate_candidate(plan, candidate, slot)?;
        } else {
            let unused = !entry.stage.is_sensitivity()
                && !entry.stage.is_statistical()
                && entry
                    .iteration
                    .is_some_and(|attempt| attempt >= plan.completed.get());
            let expected = if unused {
                State::Planned
            } else {
                State::Released(entry.extent.entries())
            };
            if entry.state.get() != expected {
                return Err(Error::IncompleteCoverage.into());
            }
        }
    }
    Ok(())
}
/// A final numerical owner must match its original stage even when another channel has equal values.
fn validate_candidate<P: Scalar>(
    plan: &Rc<Plan>,
    candidate: &Candidate<P>,
    slot: usize,
) -> Result<(), OperationError> {
    let entry = &plan.specifications[slot];
    let payload = candidate.outputs.first().ok_or(Error::IncompleteCoverage)?;
    let matches = match entry.stage {
        Stage::OutputCenters => owned(&payload.centers, plan, slot),
        Stage::OutputTotals => payload
            .bounds
            .as_ref()
            .is_some_and(|v| owned(v, plan, slot)),
        Stage::OutputErrors => payload
            .evaluation
            .as_ref()
            .is_some_and(|v| owned(v, plan, slot)),
        Stage::OutputJacobianCenters => candidate
            .statistics
            .first()
            .is_some_and(|v| owned(&v.jacobian.centers, plan, slot)),
        Stage::OutputJacobianTruth => candidate
            .statistics
            .first()
            .is_some_and(|v| owned(&v.jacobian.truth, plan, slot)),
        Stage::ContractionCenters | Stage::ContractionTruth => {
            candidate.statistics.first().is_some_and(|payload| {
                let matrix = match entry.iteration {
                    Some(0) => Some(&payload.sensitivity),
                    Some(2) => Some(&payload.covariance),
                    Some(3) => payload.means.as_ref(),
                    _ => None,
                };
                matrix.is_some_and(|matrix| {
                    if entry.stage == Stage::ContractionCenters {
                        owned(&matrix.centers, plan, slot)
                    } else {
                        owned(&matrix.truth, plan, slot)
                    }
                })
            })
        }
        _ => false,
    };
    if entry.state.get() != State::Candidate || !matches {
        return Err(Error::UnticketedBuffer.into());
    }
    Ok(())
}
/// Enclosure and scalar owners use the same exact finite slot identity without type erasure.
fn owned<T>(values: &crate::private::Buffer<T>, plan: &Rc<Plan>, slot: usize) -> bool {
    values.len() == plan.specifications[slot].extent.entries() && values.matches_solve(plan, slot)
}

/// Equal shapes or values do not replace original value and companion declarations.
fn same_symbols(left: &nrz_bindings::BindingSymbols, right: &nrz_bindings::BindingSymbols) -> bool {
    left.value() == right.value() && left.bound() == right.bound() && left.error() == right.error()
}
