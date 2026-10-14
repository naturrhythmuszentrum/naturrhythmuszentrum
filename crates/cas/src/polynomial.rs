// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded finite coefficient maps reconstructed from graph syntax.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{LogicalId, State, size};
use crate::model::{Constant, Operator};
use crate::number::Rational;

/// Stable formal indeterminates; named constants are never approximated.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Atom {
    /// A declared free identity.
    Free(String),
    /// Pi or E under an explicit stable tag.
    Constant(u8),
}

/// A sorted finite exponent vector with zero exponents omitted.
type Monomial = Vec<(Atom, u64)>;

/// A temporary finite jet, never authoritative expression storage.
#[derive(Clone, Debug)]
struct Polynomial {
    /// Nonzero coefficients in canonical monomial order.
    terms: BTreeMap<Monomial, Rational>,
}

/// A rational function view; domain obligations are checked independently.
#[derive(Clone, Debug)]
struct Fraction {
    /// Polynomial numerator.
    numerator: Polynomial,
    /// Polynomial denominator.
    denominator: Polynomial,
}

/// Shared location for finite coefficient arithmetic.
const AT: Location = Location::new("finite coefficient jets");

/// Refuse grammar outside this exact executable fragment.
const fn fragment() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

impl Polynomial {
    /// Form one checked coefficient, omitting zero.
    fn constant(value: &Rational, budget: &mut Budget) -> Result<Self, Fault> {
        value.check(budget, AT)?;
        let mut result = Self {
            terms: BTreeMap::new(),
        };
        result.accumulate(Vec::new(), value, budget)?;
        Ok(result)
    }

    /// Construct a degree-one formal indeterminate.
    fn atom(value: Atom, budget: &mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::PolynomialVariables, 1, AT)?;
        budget.check(BudgetKind::PolynomialDegree, 1, AT)?;
        let mut result = Self {
            terms: BTreeMap::new(),
        };
        result.accumulate(vec![(value, 1)], &Rational::one(), budget)?;
        Ok(result)
    }

    /// Measure retained finite-map payload, including every atom name and exact component.
    fn bytes(&self, budget: &mut Budget) -> Result<u64, Fault> {
        budget.work(size(self.terms.len(), AT)?, AT)?;
        let mut bytes = 64_u64;
        for (monomial, coefficient) in &self.terms {
            bytes = bytes
                .checked_add(entry_bytes(monomial, coefficient)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        Ok(bytes)
    }

    /// At most eight simultaneous arithmetic maps share the remaining scratch allowance.
    fn prepay(&self, budget: &mut Budget) -> Result<(), Fault> {
        let bytes = self.bytes(budget)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes
                .checked_mul(8)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        budget.work(bytes, AT)
    }

    /// Bound the union of variables across all monomials, including a pending insertion.
    fn variables(&self, monomial: &[(Atom, u64)], budget: &mut Budget) -> Result<(), Fault> {
        let mut variables = BTreeSet::new();
        for (atom, _) in self
            .terms
            .keys()
            .flat_map(|term| term.iter())
            .chain(monomial)
        {
            budget.work(
                size(variables.len(), AT)?
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(budget.limits().name_bytes))
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            let count = size(variables.len(), AT)?
                .checked_add(u64::from(!variables.contains(atom)))
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialVariables, AT))?;
            budget.check(BudgetKind::PolynomialVariables, count, AT)?;
            variables.insert(atom);
        }
        Ok(())
    }

    /// Add one coefficient, charging ordered-container comparisons before insertion.
    fn accumulate(
        &mut self,
        monomial: Monomial,
        coefficient: &Rational,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        self.prepay(budget)?;
        coefficient.check(budget, AT)?;
        self.variables(&monomial, budget)?;
        let peak = self
            .bytes(budget)?
            .checked_add(entry_bytes(&monomial, coefficient)?)
            .and_then(|n| n.checked_mul(8))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, peak, AT)?;
        let value = if let Some(previous) = self.terms.get(&monomial) {
            previous.add(coefficient, budget)?
        } else {
            coefficient.clone()
        };
        if value.is_zero() {
            self.terms.remove(&monomial);
        } else {
            let count = size(self.terms.len(), AT)?
                .checked_add(u64::from(!self.terms.contains_key(&monomial)))
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
            budget.check(BudgetKind::PolynomialTerms, count, AT)?;
            self.terms.insert(monomial, value);
        }
        Ok(())
    }

    /// Add maps without retaining zero coefficients.
    fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        other.prepay(budget)?;
        let mut result = self.clone();
        for (monomial, value) in &other.terms {
            result.accumulate(monomial.clone(), value, budget)?;
        }
        Ok(result)
    }

    /// Negate every coefficient under the arithmetic ledger.
    fn negated(&self, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        let mut result = Self {
            terms: BTreeMap::new(),
        };
        for (monomial, value) in &self.terms {
            result.accumulate(monomial.clone(), &value.negated(budget)?, budget)?;
        }
        Ok(result)
    }

    /// Multiply finite maps by exact convolution.
    fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        other.prepay(budget)?;
        let mut result = Self {
            terms: BTreeMap::new(),
        };
        for (left, a) in &self.terms {
            for (right, b) in &other.terms {
                let monomial = multiply_monomials(left, right, budget)?;
                result.accumulate(monomial, &a.mul(b, budget)?, budget)?;
            }
        }
        Ok(result)
    }

    /// Raise a polynomial using finite repeated squaring.
    fn pow(&self, exponent: u64, budget: &mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::PolynomialDegree, exponent, AT)?;
        self.prepay(budget)?;
        let mut result = Self::constant(&Rational::one(), budget)?;
        let mut factor = self.clone();
        let mut remaining = exponent;
        while remaining != 0 {
            budget.arithmetic(1, AT)?;
            if remaining & 1 == 1 {
                result = result.mul(&factor, budget)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                factor = factor.mul(&factor, budget)?;
            }
        }
        Ok(result)
    }

    /// Whether this map is a nonzero rational scalar.
    fn is_scalar(&self) -> bool {
        self.terms.len() == 1 && self.terms.keys().all(Vec::is_empty)
    }
}

/// Conservative bytes in one owned monomial/coefficient pair.
fn entry_bytes(monomial: &[(Atom, u64)], coefficient: &Rational) -> Result<u64, Fault> {
    let (numerator, denominator) = coefficient.component_bits();
    let mut bytes = numerator
        .checked_add(denominator)
        .and_then(|n| n.checked_add(15))
        .and_then(|n| n.checked_div(8))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    for (atom, _) in monomial {
        let name = match atom {
            Atom::Free(key) => size(key.len(), AT)?,
            Atom::Constant(_) => 1,
        };
        bytes = bytes
            .checked_add(name)
            .and_then(|n| n.checked_add(64))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    }
    Ok(bytes)
}

/// Merge two sorted exponent vectors and check total degree and distinct variables.
fn multiply_monomials(
    left: &[(Atom, u64)],
    right: &[(Atom, u64)],
    budget: &mut Budget,
) -> Result<Monomial, Fault> {
    let mut exponents = BTreeMap::new();
    let mut degree = 0_u64;
    for (atom, exponent) in left.iter().chain(right) {
        let work = size(exponents.len(), AT)?
            .checked_add(1)
            .and_then(|n| n.checked_mul(budget.limits().name_bytes))
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        budget.work(work, AT)?;
        let previous: u64 = exponents.get(atom).copied().unwrap_or(0);
        let combined = previous
            .checked_add(*exponent)
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        degree = degree
            .checked_add(*exponent)
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
        exponents.insert(atom.clone(), combined);
        budget.check(
            BudgetKind::PolynomialVariables,
            size(exponents.len(), AT)?,
            AT,
        )?;
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(exponents.len())
        .map_err(|_| Fault::allocation(AT))?;
    result.extend(exponents);
    Ok(result)
}

impl Fraction {
    /// Embed a polynomial into rational functions.
    fn polynomial(numerator: Polynomial, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            numerator,
            denominator: Polynomial::constant(&Rational::one(), budget)?,
        })
    }

    /// Reconstruct a binary rational operation without canceling domain information.
    fn binary(
        &self,
        other: &Self,
        operator: &Operator,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        match operator {
            Operator::Add | Operator::Sub => {
                let left = self.numerator.mul(&other.denominator, budget)?;
                let mut right = other.numerator.mul(&self.denominator, budget)?;
                if matches!(operator, Operator::Sub) {
                    right = right.negated(budget)?;
                }
                Ok(Self {
                    numerator: left.add(&right, budget)?,
                    denominator: self.denominator.mul(&other.denominator, budget)?,
                })
            }
            Operator::Mul => Ok(Self {
                numerator: self.numerator.mul(&other.numerator, budget)?,
                denominator: self.denominator.mul(&other.denominator, budget)?,
            }),
            Operator::Div => Ok(Self {
                numerator: self.numerator.mul(&other.denominator, budget)?,
                denominator: self.denominator.mul(&other.numerator, budget)?,
            }),
            _ => Err(fragment()),
        }
    }
}

/// Convert a scalar DAG with an explicit postorder stack and no success cache from search.
fn convert(state: &State, root: LogicalId, budget: &mut Budget) -> Result<Fraction, Fault> {
    let mut values: BTreeMap<LogicalId, Fraction> = BTreeMap::new();
    let mut pending = vec![(root, false)];
    let mut retained = 0_u64;
    while let Some((id, expanded)) = pending.pop() {
        budget.work(
            size(values.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if values.contains_key(&id) {
            continue;
        }
        let term = state.expression(id)?;
        if !expanded {
            pending
                .try_reserve(
                    term.operands
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
                )
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((id, true));
            pending.extend(term.operands.iter().rev().map(|child| (*child, false)));
            continue;
        }
        let previous = budget.reserve_temporary(retained, AT)?;
        let value = convert_node(&term.operator, &term.operands, &values, budget);
        budget.restore_limits(previous);
        let value = value?;
        let denominator_bytes = value.denominator.bytes(budget)?;
        retained = retained
            .checked_add(value.numerator.bytes(budget)?)
            .and_then(|n| n.checked_add(denominator_bytes))
            .and_then(|n| n.checked_add(64))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.check(
            BudgetKind::Vertices,
            size(values.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
            AT,
        )?;
        values.insert(id, value);
    }
    values.remove(&root).ok_or_else(fragment)
}

/// Reconstruct exactly one operator from previously checked coefficient views.
fn convert_node(
    operator: &Operator,
    operands: &[LogicalId],
    values: &BTreeMap<LogicalId, Fraction>,
    budget: &mut Budget,
) -> Result<Fraction, Fault> {
    let child = |position: usize| {
        operands
            .get(position)
            .and_then(|id| values.get(id))
            .ok_or_else(fragment)
    };
    match operator {
        Operator::Rational(value) => {
            Fraction::polynomial(Polynomial::constant(value, budget)?, budget)
        }
        Operator::Free(key) => {
            Fraction::polynomial(Polynomial::atom(Atom::Free(key.clone()), budget)?, budget)
        }
        Operator::Constant(value) => {
            let tag = match value {
                Constant::Pi => 0,
                Constant::E => 1,
            };
            Fraction::polynomial(Polynomial::atom(Atom::Constant(tag), budget)?, budget)
        }
        Operator::Neg => {
            let value = child(0)?;
            value.denominator.prepay(budget)?;
            Ok(Fraction {
                numerator: value.numerator.negated(budget)?,
                denominator: value.denominator.clone(),
            })
        }
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => {
            child(0)?.binary(child(1)?, operator, budget)
        }
        Operator::IntegerPower(exponent) => {
            let value = child(0)?;
            let (numerator, denominator) = if *exponent < 0 {
                (&value.denominator, &value.numerator)
            } else {
                (&value.numerator, &value.denominator)
            };
            Ok(Fraction {
                numerator: numerator.pow(exponent.unsigned_abs(), budget)?,
                denominator: denominator.pow(exponent.unsigned_abs(), budget)?,
            })
        }
        _ => Err(fragment()),
    }
}

/// Compare exact coefficient maps after independently reconstructing both endpoints.
pub(crate) fn equal(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    rational: bool,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let a = convert(state, left, budget)?;
    let b = convert(state, right, budget)?;
    if !rational && (!a.denominator.is_scalar() || !b.denominator.is_scalar()) {
        return Err(fragment());
    }
    if a.denominator.terms.is_empty() || b.denominator.terms.is_empty() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    let first = a.numerator.mul(&b.denominator, budget)?;
    let second = b.numerator.mul(&a.denominator, budget)?;
    first.prepay(budget)?;
    second.prepay(budget)?;
    Ok(first.terms == second.terms)
}
