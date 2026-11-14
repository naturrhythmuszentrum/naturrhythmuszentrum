// SPDX-License-Identifier: AGPL-3.0-only
//! Fallible ownership reservations charged by actual capacity.

use crate::{CaseRequest, recipe::RecipeError};

/// Aggregate allowance for allocations that coexist under one owner.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) struct Budget {
    /// Uncommitted bytes, excluding caller storage and allocator metadata.
    remaining: u64,
}

impl Budget {
    /// Begin an explicitly bounded allocation group.
    pub(crate) const fn new(bytes: u64) -> Self {
        Self { remaining: bytes }
    }

    /// Admit a fixed record or an actual allocation capacity.
    pub(crate) fn charge(&mut self, bytes: usize) -> Result<(), RecipeError> {
        self.remaining = self
            .remaining
            .checked_sub(u64::try_from(bytes).map_err(|_| RecipeError::Overflow)?)
            .ok_or(RecipeError::Allocation)?;
        Ok(())
    }

    /// Remaining allowance may be handed to a nested owner, never reused concurrently.
    pub(crate) const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Refuse the requested size before allocation and charge the returned capacity.
    pub(crate) fn reserve<T>(&mut self, length: usize) -> Result<Vec<T>, RecipeError> {
        let requested = length
            .checked_mul(size_of::<T>())
            .ok_or(RecipeError::Overflow)?;
        if u64::try_from(requested).map_err(|_| RecipeError::Overflow)? > self.remaining {
            return Err(RecipeError::Allocation);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| RecipeError::Allocation)?;
        self.charge(
            values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(RecipeError::Overflow)?,
        )?;
        Ok(values)
    }

    /// Copy owned text with a checked capacity before writing any bytes.
    pub(crate) fn text(&mut self, value: &str) -> Result<String, RecipeError> {
        if u64::try_from(value.len()).map_err(|_| RecipeError::Overflow)? > self.remaining {
            return Err(RecipeError::Allocation);
        }
        let mut output = String::new();
        output
            .try_reserve_exact(value.len())
            .map_err(|_| RecipeError::Allocation)?;
        self.charge(output.capacity())?;
        output.push_str(value);
        Ok(output)
    }

    /// Copy only after the complete destination capacity has been admitted.
    pub(crate) fn copy<T: Copy>(&mut self, values: &[T]) -> Result<Vec<T>, RecipeError> {
        let mut output = self.reserve(values.len())?;
        output.extend_from_slice(values);
        Ok(output)
    }
}

/// Clone numerical recipe ownership without infallible nested vector growth.
/// The enclosing owner charges the fixed `CaseRequest` record separately.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn clone_case(
    case: &CaseRequest,
    budget: &mut Budget,
) -> Result<CaseRequest, RecipeError> {
    let shape = budget.copy(&case.shape)?;
    let allowed_cpus = budget.copy(&case.allowed_cpus)?;
    let cycles = budget.copy(&case.cycles)?;
    let mut inputs = budget.reserve(case.inputs.len())?;
    for input in &case.inputs {
        inputs.push(budget.copy(input)?);
    }
    Ok(CaseRequest {
        operation: case.operation,
        width: case.width,
        dependency: case.dependency,
        scope: case.scope,
        shape,
        item_length: case.item_length,
        workers: case.workers,
        placement: case.placement,
        allowed_cpus,
        budget_ns: case.budget_ns,
        diagonal_margin: case.diagonal_margin,
        inputs,
        cycles,
    })
}

/// Own the exact request before any workload begins; caller capacities are excluded.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn clone_request(
    request: &crate::RunRequest,
    budget: &mut Budget,
) -> Result<crate::RunRequest, RecipeError> {
    budget.charge(size_of::<crate::RunRequest>())?;
    let run_id = budget.text(&request.run_id)?;
    let seeds = budget.copy(&request.seeds)?;
    let mut cases = budget.reserve(request.cases.len())?;
    for case in &request.cases {
        cases.push(clone_case(case, budget)?);
    }
    Ok(crate::RunRequest {
        version: request.version,
        run_id,
        cases,
        seeds,
        sampling: request.sampling,
        resources: request.resources,
    })
}

#[cfg(test)]
mod tests {
    use super::{Budget, RecipeError};

    #[test]
    fn simultaneous_reservations_consume_one_allowance() -> Result<(), RecipeError> {
        let mut budget = Budget::new(24);
        let first = budget.copy(&[1_u64, 2])?;
        assert_eq!(first, [1, 2]);
        assert_eq!(
            budget.remaining(),
            24 - u64::try_from(first.capacity() * 8).map_err(|_| RecipeError::Overflow)?
        );
        assert_eq!(budget.reserve::<u64>(2), Err(RecipeError::Allocation));
        let second = budget.copy(&[3_u64])?;
        assert_eq!(second, [3]);
        assert_eq!(budget.remaining(), 0);
        Ok(())
    }

    #[test]
    fn overflowing_population_is_refused_before_allocation() {
        assert_eq!(
            Budget::new(u64::MAX).reserve::<u64>(usize::MAX),
            Err(RecipeError::Overflow)
        );
    }
}
