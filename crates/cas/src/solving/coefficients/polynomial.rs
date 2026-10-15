// SPDX-License-Identifier: AGPL-3.0-only
//! Sparse ground-field polynomials in a fixed ordered formal variable frame.

use super::scalar::{Ground, Scalar};
use super::storage::{AT, add, begin, copied, invalid, product, vector};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;
use std::cmp::Ordering;

/// One scratch monomial, owned only after its reservations have succeeded.
#[derive(Debug)]
pub(crate) struct Term {
    /// Fixed-width nonnegative exponents.
    pub(crate) powers: Vec<u32>,
    /// A ground-field coefficient.
    pub(crate) coefficient: Scalar,
}

/// Canonical decreasing graded-lex polynomial; the empty vector is zero.
#[derive(Debug)]
pub(crate) struct Polynomial {
    /// Ordered frame width, including unused variables.
    pub(crate) variables: usize,
    /// Exact ground field.
    pub(crate) ground: Ground,
    /// Distinct, nonzero terms in canonical order.
    pub(crate) terms: Vec<Term>,
}

impl Term {
    /// Copy both key and scalar through the active ledger.
    pub(crate) fn copy(&self, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            powers: copied(&self.powers, budget)?,
            coefficient: self.coefficient.copy(budget)?,
        })
    }

    /// Total degree is checked before any key comparison or product allocation.
    fn degree(&self) -> Result<u64, Fault> {
        self.powers.iter().try_fold(0_u64, |total, value| {
            total
                .checked_add(u64::from(*value))
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))
        })
    }
}

/// Bound retained scalar limbs, exponent vectors and three simultaneous merge buffers before allocation.
fn preflight(count: usize, variables: usize, ground: Ground, budget: &Budget) -> Result<(), Fault> {
    let count = size(count, AT)?;
    let width = size(variables, AT)?;
    budget.check(BudgetKind::PolynomialTerms, count, AT)?;
    budget.check(BudgetKind::PolynomialVariables, width, AT)?;
    let limbs = budget.limits().component_bits.div_ceil(64);
    let components = if ground == Ground::Rational { 2 } else { 4 };
    let scalar = product(product(limbs.max(1), 8)?, components)?;
    let entry = add(
        size(std::mem::size_of::<Term>(), AT)?,
        add(product(width, 4)?, scalar)?,
    )?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        product(product(count, entry)?, 3)?,
        AT,
    )
}

/// Charge every inspected exponent position, with decreasing total-degree/lex order.
fn compare(left: &Term, right: &Term, budget: &mut Budget) -> Result<Ordering, Fault> {
    budget.work(product(size(left.powers.len(), AT)?.max(1), 3)?, AT)?;
    Ok((right.degree()?, &right.powers).cmp(&(left.degree()?, &left.powers)))
}

/// One deterministic merge pass; no library sort hides allocation or comparisons.
fn merge_pass(terms: &[Term], width: usize, budget: &mut Budget) -> Result<Vec<Term>, Fault> {
    let mut result = vector(terms.len(), budget)?;
    let stride = width
        .checked_mul(2)
        .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
    for start in (0..terms.len()).step_by(stride) {
        let middle = start.saturating_add(width).min(terms.len());
        let end = start.saturating_add(stride).min(terms.len());
        let mut left = start;
        let mut right = middle;
        while left < middle || right < end {
            let take_left = right == end
                || (left < middle
                    && compare(
                        terms.get(left).ok_or_else(invalid)?,
                        terms.get(right).ok_or_else(invalid)?,
                        budget,
                    )? != Ordering::Greater);
            let index = if take_left {
                let slot = left;
                left += 1;
                slot
            } else {
                let slot = right;
                right += 1;
                slot
            };
            result.push(terms.get(index).ok_or_else(invalid)?.copy(budget)?);
        }
    }
    Ok(result)
}

/// Merge adjacent equal monomials without estimating space from final cancellation.
fn collect(terms: Vec<Term>, budget: &mut Budget) -> Result<Vec<Term>, Fault> {
    let mut result: Vec<Term> = vector(terms.len(), budget)?;
    for term in terms {
        if let Some(last) = result.last_mut() {
            budget.work(size(term.powers.len(), AT)?.max(1), AT)?;
            if last.powers == term.powers {
                last.coefficient = last.coefficient.add(&term.coefficient, budget)?;
                if last.coefficient.zero(budget)? {
                    let _removed = result.pop();
                }
                continue;
            }
        }
        if !term.coefficient.zero(budget)? {
            result.push(term);
        }
    }
    Ok(result)
}

impl Polynomial {
    /// Validate, merge-sort, combine and remove zero terms under explicit bounds.
    pub(crate) fn new(
        variables: usize,
        ground: Ground,
        mut terms: Vec<Term>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        begin(budget)?;
        preflight(terms.len(), variables, ground, budget)?;
        for term in &terms {
            budget.work(size(variables, AT)?.max(1), AT)?;
            if term.powers.len() != variables || term.coefficient.ground() != ground {
                return Err(invalid());
            }
            budget.check(BudgetKind::PolynomialDegree, term.degree()?, AT)?;
        }
        let mut width = 1_usize;
        while width < terms.len() {
            terms = merge_pass(&terms, width, budget)?;
            width = width
                .checked_mul(2)
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
        }
        Ok(Self {
            variables,
            ground,
            terms: collect(terms, budget)?,
        })
    }

    /// A constant uses the full variable frame even when it is zero.
    pub(crate) fn constant(
        variables: usize,
        value: Scalar,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let ground = value.ground();
        preflight(1, variables, ground, budget)?;
        let mut powers = vector(variables, budget)?;
        powers.resize(variables, 0);
        let mut terms = vector(1, budget)?;
        terms.push(Term {
            powers,
            coefficient: value,
        });
        Self::new(variables, ground, terms, budget)
    }

    /// Construct the multiplicative identity.
    pub(crate) fn one(&self, budget: &mut Budget) -> Result<Self, Fault> {
        Self::constant(
            self.variables,
            Scalar::constant(self.ground, true, budget)?,
            budget,
        )
    }

    /// Metered scratch copying preserves canonical order.
    pub(crate) fn copy(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        preflight(self.terms.len(), self.variables, self.ground, budget)?;
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(term.copy(budget)?);
        }
        Ok(Self {
            variables: self.variables,
            ground: self.ground,
            terms,
        })
    }

    /// Type compatibility is independent of polynomial identity.
    fn compatible(&self, other: &Self) -> Result<(), Fault> {
        if self.variables != other.variables || self.ground != other.ground {
            return Err(invalid());
        }
        Ok(())
    }

    /// Formal zero does not assert a regional unit for any other polynomial.
    pub(crate) fn zero(&self, budget: &mut Budget) -> Result<bool, Fault> {
        begin(budget)?;
        Ok(self.terms.is_empty())
    }

    /// Recognize ground constants, including zero.
    pub(crate) fn scalar(&self, budget: &mut Budget) -> Result<Option<Scalar>, Fault> {
        begin(budget)?;
        if self.terms.is_empty() {
            return Ok(Some(Scalar::constant(self.ground, false, budget)?));
        }
        if let [term] = self.terms.as_slice() {
            budget.work(size(self.variables, AT)?, AT)?;
            if term.powers.iter().all(|power| *power == 0) {
                return Ok(Some(term.coefficient.copy(budget)?));
            }
        }
        Ok(None)
    }

    /// Equality scans every retained coefficient and exponent with charge.
    pub(crate) fn equals(&self, other: &Self, budget: &mut Budget) -> Result<bool, Fault> {
        begin(budget)?;
        self.compatible(other)?;
        if self.terms.len() != other.terms.len() {
            return Ok(false);
        }
        for (left, right) in self.terms.iter().zip(&other.terms) {
            if compare(left, right, budget)? != Ordering::Equal
                || !left.coefficient.equals(&right.coefficient, budget)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Addition reserves both full inputs before cancellation.
    pub(crate) fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.compatible(other)?;
        let count = add(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        budget.check(BudgetKind::PolynomialTerms, count, AT)?;
        let length =
            usize::try_from(count).map_err(|_| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
        preflight(length, self.variables, self.ground, budget)?;
        let mut terms = vector(length, budget)?;
        for term in self.terms.iter().chain(&other.terms) {
            terms.push(term.copy(budget)?);
        }
        Self::new(self.variables, self.ground, terms, budget)
    }

    /// Polynomial negation visits every ground coefficient.
    pub(crate) fn neg(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        preflight(self.terms.len(), self.variables, self.ground, budget)?;
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(Term {
                powers: copied(&term.powers, budget)?,
                coefficient: term.coefficient.neg(budget)?,
            });
        }
        Ok(Self {
            variables: self.variables,
            ground: self.ground,
            terms,
        })
    }

    /// Polynomial subtraction preserves the same reservations as addition.
    pub(crate) fn sub(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.add(&other.neg(budget)?, budget)
    }

    /// Schoolbook products reserve all term pairs before computing any pair.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.compatible(other)?;
        let count = product(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        budget.check(BudgetKind::PolynomialTerms, count, AT)?;
        let length =
            usize::try_from(count).map_err(|_| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
        preflight(length, self.variables, self.ground, budget)?;
        let mut terms = vector(length, budget)?;
        for left in &self.terms {
            for right in &other.terms {
                budget.check(
                    BudgetKind::PolynomialDegree,
                    add(left.degree()?, right.degree()?)?,
                    AT,
                )?;
                let mut powers = vector(self.variables, budget)?;
                for (a, b) in left.powers.iter().zip(&right.powers) {
                    powers.push(
                        a.checked_add(*b)
                            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?,
                    );
                }
                terms.push(Term {
                    powers,
                    coefficient: left.coefficient.mul(&right.coefficient, budget)?,
                });
            }
        }
        Self::new(self.variables, self.ground, terms, budget)
    }

    /// Divide every coefficient by a ground-field unit.
    pub(crate) fn scale_down(&self, divisor: &Scalar, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        preflight(self.terms.len(), self.variables, self.ground, budget)?;
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(Term {
                powers: copied(&term.powers, budget)?,
                coefficient: term.coefficient.div(divisor, budget)?,
            });
        }
        Ok(Self {
            variables: self.variables,
            ground: self.ground,
            terms,
        })
    }

    /// Normalize the graded-lex leading coefficient to one.
    pub(crate) fn monic(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        match self.terms.first() {
            Some(term) => self.scale_down(&term.coefficient, budget),
            None => self.copy(budget),
        }
    }

    /// A formal power with bounded repeated squaring.
    pub(crate) fn pow(&self, exponent: u32, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        budget.check(BudgetKind::IntegerExponent, u64::from(exponent), AT)?;
        let mut result = self.one(budget)?;
        let mut factor = self.copy(budget)?;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, budget)?;
            }
        }
        Ok(result)
    }
}
