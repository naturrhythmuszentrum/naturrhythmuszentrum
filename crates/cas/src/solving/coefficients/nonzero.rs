// SPDX-License-Identifier: AGPL-3.0-only
//! Checked multiplicative closure of already proved regional nonzero polynomials.

use super::polynomial::Polynomial;
use super::scalar::Ground;
use super::storage::{AT, begin, invalid, reserve, vector};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Handle, State, size};
use crate::proof_model::Statement;

/// Only proved, defined factors in the queried polynomial's exact ring enter this set.
pub(crate) struct Factors {
    /// Ordered parameter frame width.
    variables: usize,
    /// Exact scalar field.
    ground: Ground,
    /// Nonconstant polynomials with checked nonvanishing evidence.
    values: Vec<Polynomial>,
}

impl Factors {
    /// Ground factors need no division; duplicate guards add no search choices.
    pub(crate) fn insert(&mut self, value: Polynomial, budget: &mut Budget) -> Result<(), Fault> {
        begin(budget)?;
        if (value.variables, value.ground) != (self.variables, self.ground)
            || value.scalar(budget)?.is_some()
        {
            return Ok(());
        }
        budget.solving(
            BudgetKind::DenominatorFactors,
            super::storage::add(size(self.values.len(), AT)?, 1)?,
            AT,
        )?;
        for earlier in &self.values {
            if value.equals(earlier, budget)? {
                return Ok(());
            }
        }
        reserve(&mut self.values, 1, budget)?;
        self.values.push(value);
        Ok(())
    }

    /// Explore alternative exact divisions; overlapping proved factors must not make order decisive.
    fn reduces(&self, polynomial: &Polynomial, budget: &mut Budget) -> Result<bool, Fault> {
        let mut pending = vector(1, budget)?;
        pending.push((polynomial.copy(budget)?, 0));
        while let Some((remaining, start)) = pending.pop() {
            begin(budget)?;
            if let Some(constant) = remaining.scalar(budget)? {
                if !constant.zero(budget)? {
                    return Ok(true);
                }
                continue;
            }
            let degree = remaining.total_degree(budget)?.ok_or_else(invalid)?;
            for (index, factor) in self.values.iter().enumerate().skip(start) {
                if factor.total_degree(budget)?.ok_or_else(invalid)? > degree {
                    continue;
                }
                if let Some(quotient) = super::gcd::exact(&remaining, factor, budget)? {
                    let next = quotient.total_degree(budget)?.ok_or_else(invalid)?;
                    if next >= degree {
                        return Err(invalid());
                    }
                    reserve(&mut pending, 1, budget)?;
                    pending.push((quotient, index));
                }
            }
        }
        Ok(false)
    }
}

/// Generation and replay supply their checked premises to the identical finite closure.
pub(crate) fn proves<'a>(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    statements: impl Iterator<Item = &'a Statement>,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    begin(budget)?;
    if let Some(constant) = polynomial.scalar(budget)? {
        return Ok(!constant.zero(budget)?);
    }
    let mut factors = Factors {
        variables: polynomial.variables,
        ground: polynomial.ground,
        values: Vec::new(),
    };
    super::regional::factors(state, source, region, &mut factors, budget)?;
    for statement in statements {
        super::nonvanishing::factors(state, source, region, statement, &mut factors, budget)?;
    }
    factors.reduces(polynomial, budget)
}
