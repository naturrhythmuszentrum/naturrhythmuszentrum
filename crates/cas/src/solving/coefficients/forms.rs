// SPDX-License-Identifier: AGPL-3.0-only
//! Independent sparse identity arithmetic over arbitrary admitted coefficient algebras.

use super::arithmetic::Engine;
use super::model::{CoefficientFailure, Element};
use super::storage::{AT, add, invalid, product, vector};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{LogicalId, size};
use std::cmp::Ordering;

/// Formal slots and actual scalar atoms are distinct variable namespaces.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Variable {
    /// An ordered bound slot in a graph polynomial.
    Slot(u32),
    /// A scoped scalar atom, identified by its canonical graph node.
    Atom(LogicalId),
}

/// One sparse monomial and its exact coefficient.
#[derive(Debug)]
pub(crate) struct Term {
    /// Increasing variable keys with positive exponents.
    pub(crate) powers: Vec<(Variable, u32)>,
    /// Coefficient in the checker's declared domain.
    pub(crate) coefficient: Element,
}

/// A canonical decreasing graded-lex polynomial used only by the replay checker.
#[derive(Debug)]
pub(crate) struct Form {
    /// No repeated or zero terms.
    pub(crate) terms: Vec<Term>,
}

/// Compare formal exponent vectors, charging the complete key scan.
fn compare(left: &Term, right: &Term, budget: &mut Budget) -> Result<Ordering, CoefficientFailure> {
    budget.work(
        add(size(left.powers.len(), AT)?, size(right.powers.len(), AT)?)?,
        AT,
    )?;
    let degree = |term: &Term| {
        term.powers.iter().try_fold(0_u64, |total, (_, exponent)| {
            add(total, u64::from(*exponent))
        })
    };
    let order = degree(left)?.cmp(&degree(right)?);
    if order != Ordering::Equal {
        return Ok(order);
    }
    let mut first = left.powers.iter().peekable();
    let mut second = right.powers.iter().peekable();
    while let (Some(a), Some(b)) = (first.peek(), second.peek()) {
        match a.0.cmp(&b.0) {
            Ordering::Less => return Ok(Ordering::Greater),
            Ordering::Greater => return Ok(Ordering::Less),
            Ordering::Equal if a.1 != b.1 => return Ok(a.1.cmp(&b.1)),
            Ordering::Equal => {
                let _first = first.next();
                let _second = second.next();
            }
        }
    }
    Ok(first.len().cmp(&second.len()))
}

impl Term {
    /// Copy an exponent key and coefficient with explicit work and storage checks.
    fn copy(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, CoefficientFailure> {
        Ok(Self {
            powers: super::storage::copied(&self.powers, budget)?,
            coefficient: engine.copy(&self.coefficient, budget)?,
        })
    }

    /// Checked sparse monomial multiplication in the fixed variable order.
    fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut powers = super::storage::copied(&self.powers, budget)?;
        super::storage::reserve(&mut powers, other.powers.len(), budget)?;
        for (variable, exponent) in &other.powers {
            budget.work(size(powers.len(), AT)?.max(1), AT)?;
            match powers.binary_search_by_key(variable, |entry| entry.0) {
                Ok(index) => {
                    let entry = powers.get_mut(index).ok_or_else(invalid)?;
                    entry.1 = entry.1.checked_add(*exponent).ok_or_else(|| {
                        crate::error::Fault::capacity(BudgetKind::PolynomialDegree, AT)
                    })?;
                }
                Err(index) => powers.insert(index, (*variable, *exponent)),
            }
        }
        let degree = powers.iter().try_fold(0_u64, |total, (_, exponent)| {
            add(total, u64::from(*exponent))
        })?;
        budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
        budget.check(BudgetKind::PolynomialVariables, size(powers.len(), AT)?, AT)?;
        Ok(Self {
            powers,
            coefficient: engine.mul(&self.coefficient, &other.coefficient, budget)?,
        })
    }
}

impl Form {
    /// A constant, possibly zero, in the exact source algebra.
    pub(crate) fn constant(
        value: Element,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(1, budget)?;
        if !engine.formal_zero(&value, budget)? {
            terms.push(Term {
                powers: Vec::new(),
                coefficient: value,
            });
        }
        Ok(Self { terms })
    }

    /// A formal variable with unit coefficient.
    pub(crate) fn variable(
        variable: Variable,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut powers = vector(1, budget)?;
        powers.push((variable, 1));
        let mut terms = vector(1, budget)?;
        terms.push(Term {
            powers,
            coefficient: engine.constant(true, budget)?,
        });
        Ok(Self { terms })
    }

    /// Copy all terms with charge.
    pub(crate) fn copy(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(term.copy(engine, budget)?);
        }
        Ok(Self { terms })
    }

    /// Merge two sorted lists, reserving the uncancelled output before any coefficient work.
    pub(crate) fn add(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let count = add(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        budget.check(BudgetKind::PolynomialTerms, count, AT)?;
        let mut terms = vector(usize::try_from(count).map_err(|_| invalid())?, budget)?;
        let mut left = self.terms.iter().peekable();
        let mut right = other.terms.iter().peekable();
        while let (Some(a), Some(b)) = (left.peek(), right.peek()) {
            match compare(a, b, budget)? {
                Ordering::Greater => {
                    terms.push(a.copy(engine, budget)?);
                    let _left = left.next();
                }
                Ordering::Less => {
                    terms.push(b.copy(engine, budget)?);
                    let _right = right.next();
                }
                Ordering::Equal => {
                    let coefficient = engine.add(&a.coefficient, &b.coefficient, budget)?;
                    if !engine.formal_zero(&coefficient, budget)? {
                        terms.push(Term {
                            powers: super::storage::copied(&a.powers, budget)?,
                            coefficient,
                        });
                    }
                    let _left = left.next();
                    let _right = right.next();
                }
            }
        }
        for term in left.chain(right) {
            terms.push(term.copy(engine, budget)?);
        }
        Ok(Self { terms })
    }

    /// Negate coefficients without changing variable order.
    pub(crate) fn neg(
        &self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let mut terms = vector(self.terms.len(), budget)?;
        for term in &self.terms {
            terms.push(Term {
                powers: super::storage::copied(&term.powers, budget)?,
                coefficient: engine.neg(&term.coefficient, budget)?,
            });
        }
        Ok(Self { terms })
    }

    /// At most mn term products and mn accumulator insertions, each using charged key comparisons.
    pub(crate) fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let pairs = product(size(self.terms.len(), AT)?, size(other.terms.len(), AT)?)?;
        budget.check(BudgetKind::PolynomialTerms, pairs, AT)?;
        budget.work(pairs, AT)?;
        let mut result = Self { terms: Vec::new() };
        for left in &self.terms {
            for right in &other.terms {
                let term = left.mul(right, engine, budget)?;
                let mut terms = vector(1, budget)?;
                if !engine.formal_zero(&term.coefficient, budget)? {
                    terms.push(term);
                }
                result = result.add(&Self { terms }, engine, budget)?;
            }
        }
        Ok(result)
    }

    /// Nonnegative formal powers with bounded repeated squaring.
    pub(crate) fn pow(
        &self,
        exponent: u32,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        budget.check(BudgetKind::IntegerExponent, u64::from(exponent), AT)?;
        let mut result = Self::constant(engine.constant(true, budget)?, engine, budget)?;
        let mut factor = self.copy(engine, budget)?;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, engine, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, engine, budget)?;
            }
        }
        Ok(result)
    }
}
