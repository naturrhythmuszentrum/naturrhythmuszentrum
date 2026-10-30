// SPDX-License-Identifier: AGPL-3.0-only
//! Checked integer operations retain their actual operands and actual result domains.

use super::{CountError, CountHistory, CountQuantity, charge};
use nrz_bindings::Budget;

impl CountQuantity {
    /// Add the same counted thing and check the actual result domain and finite capacity.
    /// # Errors
    /// Refuses different kinds/domains, integer overflow, out-of-domain results and caller resources.
    pub fn add(
        &self,
        right: &Self,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        self.descriptor().compatible_kind(right.descriptor())?;
        let value = self
            .value()
            .checked_add(right.value())
            .ok_or(CountError::Overflow)?;
        self.result(
            value,
            capacity,
            CountHistory::Add(self.clone(), right.clone()),
            budget,
        )
    }

    /// Subtract the same counted thing without wrapping a negative result into unsigned storage.
    /// # Errors
    /// Refuses different kinds/domains, negative finite results, actual domain violations and resources.
    pub fn subtract(
        &self,
        right: &Self,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        self.descriptor().compatible_kind(right.descriptor())?;
        let value = self
            .value()
            .checked_sub(right.value())
            .ok_or(CountError::Overflow)?;
        self.result(
            value,
            capacity,
            CountHistory::Subtract(self.clone(), right.clone()),
            budget,
        )
    }

    /// Scale a tally by an explicit whole-number multiplicity without inventing a new counted kind.
    /// # Errors
    /// Refuses integer overflow, actual domain violations, capacity and caller resources.
    pub fn scale(
        &self,
        factor: u64,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        let value = self
            .value()
            .checked_mul(factor)
            .ok_or(CountError::Overflow)?;
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
        divisor: u64,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        charge(budget)?;
        if divisor == 0 {
            return Err(CountError::ZeroDivisor);
        }
        if !self.value().is_multiple_of(divisor) {
            return Err(CountError::Fractional);
        }
        self.result(
            self.value() / divisor,
            capacity,
            CountHistory::Quotient(self.clone(), divisor),
            budget,
        )
    }
}
