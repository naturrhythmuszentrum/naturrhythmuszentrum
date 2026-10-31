// SPDX-License-Identifier: AGPL-3.0-only
//! Checked integer operations retain their actual operands and actual result domains.

use super::{CountError, CountHistory, CountQuantity, charge, integer};
use nrz_bindings::{BinaryOp, Budget};

impl CountQuantity {
    /// Add the same counted thing and check the actual result domain and finite capacity.
    /// # Errors
    /// Refuses different kinds/domains, integer overflow, out-of-domain results and caller resources.
    pub fn add(
        &self,
        right: &Self,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        self.descriptor().compatible_kind(right.descriptor())?;
        let Some(value) = self.value().checked_add(right.value()) else {
            return self.overflow(right.value(), BinaryOp::Add, budget);
        };
        self.result(
            value,
            capacity,
            CountHistory::Add(self.clone(), right.clone()),
            budget,
        )
    }

    /// Subtract the same counted thing while preserving its declared signed domain.
    /// # Errors
    /// Refuses different kinds/domains, signed overflow, actual domain violations and resources.
    pub fn subtract(
        &self,
        right: &Self,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        self.descriptor().compatible_kind(right.descriptor())?;
        let Some(value) = self.value().checked_sub(right.value()) else {
            return self.overflow(right.value(), BinaryOp::Subtract, budget);
        };
        self.result(
            value,
            capacity,
            CountHistory::Subtract(self.clone(), right.clone()),
            budget,
        )
    }

    /// Scale a tally by an explicit signed integer multiplicity without inventing a new counted kind.
    /// # Errors
    /// Refuses integer overflow, actual domain violations, capacity and caller resources.
    pub fn scale(
        &self,
        factor: i128,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        let Some(value) = self.value().checked_mul(factor) else {
            return self.overflow(factor, BinaryOp::Multiply, budget);
        };
        self.result(
            value,
            capacity,
            CountHistory::Scale(self.clone(), factor),
            budget,
        )
    }

    /// Partition a tally only when the exact quotient remains a count of the same thing.
    /// # Errors
    /// Refuses zero divisors, fractional results, actual domain violations, capacity and resources.
    pub fn quotient(
        &self,
        divisor: i128,
        capacity: u128,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        if divisor == 0 {
            return Err(CountError::ZeroDivisor);
        }
        let Some(value) = self.value().checked_div(divisor) else {
            return self.overflow(divisor, BinaryOp::Divide, budget);
        };
        if self.value().checked_rem(divisor) != Some(0) {
            return Err(CountError::Fractional);
        }
        self.result(
            value,
            capacity,
            CountHistory::Quotient(self.clone(), divisor),
            budget,
        )
    }

    /// Check the mathematical domain even when finite signed arithmetic cannot hold its result.
    fn overflow(
        &self,
        right: i128,
        operation: BinaryOp,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        let left = integer(self.value(), budget)?;
        let right = integer(right, budget)?;
        let exact = match operation {
            BinaryOp::Add => left.add(&right, budget)?,
            BinaryOp::Subtract => left.sub(&right, budget)?,
            BinaryOp::Multiply => left.mul(&right, budget)?,
            BinaryOp::Divide => left.div(&right, budget)?,
        };
        self.descriptor()
            .kind()
            .count_domain()
            .ok_or(nrz_bindings::UnitError::CountDomain)?
            .check_with_budget(&exact, budget)?;
        Err(CountError::Overflow)
    }
}
