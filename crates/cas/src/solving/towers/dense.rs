// SPDX-License-Identifier: AGPL-3.0-only
//! Metered univariate arithmetic over an explicitly named parent algebra.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Handle, State, size};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{CoefficientFailure, Element};
use crate::solving::coefficients::storage::{AT, add, begin, invalid, polynomial, product, vector};

/// Ascending-degree coefficients, with no trailing formal zero.
#[derive(Debug)]
pub(crate) struct Dense {
    /// Exact parent algebra shared by every entry.
    pub(crate) domain: Handle,
    /// Canonical ascending vector; empty means zero.
    pub(crate) coefficients: Vec<Element>,
}

impl Dense {
    /// Remove trailing zeros using metered formal tests, without assuming any unit.
    pub(crate) fn new(
        engine: &Engine<'_>,
        mut coefficients: Vec<Element>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        for entry in &coefficients {
            if entry.domain != *engine.domain {
                return Err(invalid().into());
            }
        }
        while let Some(last) = coefficients.last() {
            if !engine.formal_zero(last, budget)? {
                break;
            }
            let _removed = coefficients.pop();
        }
        budget.check(
            BudgetKind::PolynomialDegree,
            size(coefficients.len().saturating_sub(1), AT)?,
            AT,
        )?;
        Ok(Self {
            domain: engine.domain.clone(),
            coefficients,
        })
    }

    /// An explicit zero polynomial in the same parent algebra.
    pub(crate) fn zero(
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        Ok(Self {
            domain: engine.domain.clone(),
            coefficients: Vec::new(),
        })
    }

    /// The constant one polynomial.
    pub(crate) fn one(
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut coefficients = vector(1, budget)?;
        coefficients.push(engine.constant(true, budget)?);
        Self::new(engine, coefficients, budget)
    }

    /// Copy every recursive coefficient before returning an owned polynomial.
    pub(crate) fn copy(
        &self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let mut coefficients = vector(self.coefficients.len(), budget)?;
        for entry in &self.coefficients {
            coefficients.push(engine.copy(entry, budget)?);
        }
        Self::new(&engine, coefficients, budget)
    }

    /// Import a canonical univariate graph polynomial without relying on its display variable.
    pub(crate) fn read(
        state: &State,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        let stored = polynomial(state, handle)?;
        if stored.variables.len() != 1 {
            return Err(invalid().into());
        }
        let engine = Engine {
            state,
            domain: &stored.domain,
        };
        let count = stored.terms.first().map_or(Ok(0), |term| {
            add(u64::from(*term.exponents.first().ok_or_else(invalid)?), 1)
        })?;
        let mut coefficients = zero_vector(&engine, count, budget)?;
        for term in &stored.terms {
            let index = usize::try_from(*term.exponents.first().ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            *coefficients.get_mut(index).ok_or_else(invalid)? =
                engine.read(&term.coefficient, budget)?;
        }
        Self::new(&engine, coefficients, budget)
    }

    /// Sum or difference, reserving the complete uncancelled degree bound.
    pub(crate) fn sum(
        &self,
        other: &Self,
        subtract: bool,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        if self.domain != other.domain {
            return Err(invalid().into());
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let mut coefficients = zero_vector(
            &engine,
            size(self.coefficients.len().max(other.coefficients.len()), AT)?,
            budget,
        )?;
        for (index, output) in coefficients.iter_mut().enumerate() {
            if let Some(entry) = self.coefficients.get(index) {
                *output = engine.copy(entry, budget)?;
            }
            if let Some(entry) = other.coefficients.get(index) {
                *output = if subtract {
                    engine.sub(output, entry, budget)?
                } else {
                    engine.add(output, entry, budget)?
                };
            }
        }
        Self::new(&engine, coefficients, budget)
    }

    /// Schoolbook convolution charges each parent product and accumulator addition.
    pub(crate) fn mul(
        &self,
        other: &Self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        if self.domain != other.domain {
            return Err(invalid().into());
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        if self.coefficients.is_empty() || other.coefficients.is_empty() {
            return Self::zero(&engine, budget);
        }
        let count = add(
            size(self.coefficients.len(), AT)?,
            size(other.coefficients.len(), AT)?,
        )?
        .checked_sub(1)
        .ok_or_else(invalid)?;
        budget.work(
            product(
                size(self.coefficients.len(), AT)?,
                size(other.coefficients.len(), AT)?,
            )?,
            AT,
        )?;
        let mut coefficients = zero_vector(&engine, count, budget)?;
        for (left_index, left) in self.coefficients.iter().enumerate() {
            for (right_index, right) in other.coefficients.iter().enumerate() {
                let index = left_index
                    .checked_add(right_index)
                    .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
                let output = coefficients.get_mut(index).ok_or_else(invalid)?;
                *output = engine.add(output, &engine.mul(left, right, budget)?, budget)?;
            }
        }
        Self::new(&engine, coefficients, budget)
    }

    /// Multiply by one parent coefficient and shift without hidden coefficient division.
    pub(crate) fn scaled(
        &self,
        scalar: &Element,
        shift: usize,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        begin(budget)?;
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let count = add(size(self.coefficients.len(), AT)?, size(shift, AT)?)?;
        let mut coefficients = zero_vector(&engine, count, budget)?;
        for (index, entry) in self.coefficients.iter().enumerate() {
            let destination = index
                .checked_add(shift)
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
            *coefficients.get_mut(destination).ok_or_else(invalid)? =
                engine.mul(entry, scalar, budget)?;
        }
        Self::new(&engine, coefficients, budget)
    }

    /// Canonical equality is checked recursively through the parent's subtraction and zero test.
    pub(crate) fn equals(
        &self,
        other: &Self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        Ok(self
            .sum(other, true, state, budget)?
            .coefficients
            .is_empty())
    }
}

/// Reserve the complete degree and coefficient extent before constructing any recursive zero.
fn zero_vector(
    engine: &Engine<'_>,
    count: u64,
    budget: &mut Budget,
) -> Result<Vec<Element>, CoefficientFailure> {
    budget.check(BudgetKind::PolynomialTerms, count, AT)?;
    budget.check(BudgetKind::PolynomialDegree, count.saturating_sub(1), AT)?;
    let length =
        usize::try_from(count).map_err(|_| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
    let mut result = vector(length, budget)?;
    for _ in 0..length {
        result.push(engine.constant(false, budget)?);
    }
    Ok(result)
}
