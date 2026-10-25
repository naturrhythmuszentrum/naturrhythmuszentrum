// SPDX-License-Identifier: AGPL-3.0-only
//! Certified live coordinate execution commits a complete child schedule atomically.

use super::{CoordinateError, CoordinateReading, PiCoordinateContract, ReadingBinding, input};
use crate::{Heap, HeapError};
use nrz_bindings::{
    Budget, Limits, LimitsOverride,
    readings::{
        CoordinateDirection,
        algebra::{Location, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

impl Heap {
    /// Normalize one live reading through already bound pi coefficient regions.
    ///
    /// The execution policy bounds exact input checks and numerical arrays separately
    /// from the certificate owner. Each consumer retains its own selected revisions.
    /// # Errors
    /// Refuses foreign graphs, changed definitions, stale inputs, unsupported companions,
    /// numerical overflow and all explicit resources without changing caller usage.
    pub fn normalize_certified_reading<P: Scalar + 'static>(
        &self,
        binding: &ReadingBinding<P>,
        contract: &PiCoordinateContract<P>,
        execution: &LimitsOverride,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CoordinateReading<P>, CoordinateError> {
        contract.check(binding, context)?;
        let lowered = contract.lowered(false);
        let bytes = super::resources::normalization(binding, contract.contract(), &lowered)?;
        atomic(contract, execution, bytes, budget, |local| {
            self.validate_reading(binding)?;
            let indices = input::check(binding, contract.contract(), local)?;
            local.work(1, AT)?;
            self.finish_normalize(
                binding,
                contract.contract(),
                lowered,
                &indices,
                context,
                local,
            )
        })
    }

    /// Restore all value/error arrays and both covariance axes through the same bound pi owner.
    /// # Errors
    /// Refuses a different certificate, repeated restoration, foreign or stale inputs,
    /// changed arithmetic and exhausted resources without publishing a partial result.
    pub fn restore_certified_reading<P: Scalar + 'static>(
        &self,
        input: &CoordinateReading<P>,
        contract: &PiCoordinateContract<P>,
        execution: &LimitsOverride,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CoordinateReading<P>, CoordinateError> {
        contract.check(input.source(), context)?;
        let direction = input.direction() == CoordinateDirection::Normalize;
        let same_node = Rc::ptr_eq(&input.0.lowered, &contract.lowered(false));
        let same_revision = input.0.revision == context.revision();
        if !(direction && same_node && same_revision) {
            return Err(CoordinateError::Identity);
        }
        let lowered = contract.lowered(true);
        let bytes = super::resources::restoration(input, &lowered)?;
        atomic(contract, execution, bytes, budget, |local| {
            self.validate_reading(input.source())?;
            local.work(1, AT)?;
            Self::finish_restore(input, lowered, context, local)
        })
    }
}

/// Execution counts one use of an already authenticated immutable coefficient node.
const AT: Location = Location::new("certified coordinate execution");

/// Reserve the planned actual schedule and intersect child ceilings before any numerical allocation.
fn atomic<P: Scalar + 'static, T>(
    contract: &PiCoordinateContract<P>,
    policy: &LimitsOverride,
    bytes: super::resources::ExecutionBytes,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, CoordinateError>,
) -> Result<T, CoordinateError> {
    let mut limits = Limits::default().resolve(policy)?;
    let retained = contract
        .owned_bytes()
        .checked_add(
            u64::try_from(contract.constant().owned_bytes())
                .map_err(|_| HeapError::ExtentOverflow)?,
        )
        .ok_or(HeapError::ExtentOverflow)?;
    let staged = bytes
        .staged
        .checked_add(retained)
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, staged, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes.output, AT)?;
    limits.staged_payload_bytes = limits.staged_payload_bytes.min(bytes.staged);
    limits.output_bytes = limits.output_bytes.min(bytes.output);
    limits.work = limits
        .work
        .min(budget.limits().work.saturating_sub(budget.usage().work));
    let mut local = Budget::new(limits)?;
    let result = operation(&mut local)?;
    budget.work(local.usage().work, AT)?;
    Ok(result)
}
