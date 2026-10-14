// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded finite coefficient maps reconstructed from graph syntax.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, State, size};
use crate::model::{Constant, Operator, Sort};
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

/// Generate a coefficient expression; the caller still owes an independent equality proof.
pub(crate) fn normalize(
    cas: &mut Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    normalize_mode(cas, root, false, budget)
}

/// Keep composite denominator syntax when differentiation would otherwise invent extra guards.
pub(crate) fn normalize_derivative(
    cas: &mut Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    normalize_mode(cas, root, true, budget)
}

/// A mathematical producer may choose a conservative canonicalization fragment.
fn normalize_mode(
    cas: &mut Cas,
    root: LogicalId,
    preserve_denominator: bool,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let previous = budget.reserve_temporary(cas.counts().payload_bytes, AT)?;
    let converted = convert(&cas.state, root, budget);
    budget.restore_limits(previous);
    let value = converted?;
    if preserve_denominator
        && value.denominator.terms.len() > 1
        && !value.numerator.terms.is_empty()
    {
        return Ok(root);
    }
    if value.denominator.terms.is_empty() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    let numerator_bytes = value.numerator.bytes(budget)?;
    let bytes = numerator_bytes
        .checked_add(value.denominator.bytes(budget)?)
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    let previous = budget.reserve_temporary(bytes, AT)?;
    let result = materialize_fraction(cas, root, value, budget);
    budget.restore_limits(previous);
    result
}

/// Retained maps reserve scratch space while materialization grows the private graph.
fn materialize_fraction(
    cas: &mut Cas,
    root: LogicalId,
    mut value: Fraction,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    cancel_monomial(&mut value, budget)?;
    let scale = value
        .denominator
        .terms
        .values()
        .next_back()
        .ok_or_else(fragment)?
        .clone();
    for polynomial in [&mut value.numerator, &mut value.denominator] {
        polynomial.prepay(budget)?;
        for coefficient in polynomial.terms.values_mut() {
            *coefficient = coefficient.div(&scale, budget)?;
        }
    }
    let expression = cas.state.expression(root)?;
    let context = expression.context;
    let sort = expression.sort.clone();
    let numerator = materialize(cas, context, &sort, &value.numerator, budget)?;
    if value.denominator.is_scalar() || value.numerator.terms.is_empty() {
        return Ok(numerator);
    }
    let denominator = materialize(cas, context, &sort, &value.denominator, budget)?;
    cas.generated(
        context,
        Operator::Div,
        sort,
        &[numerator, denominator],
        budget,
    )
}

/// Remove only a monomial common to every numerator and denominator term.
fn cancel_monomial(value: &mut Fraction, budget: &mut Budget) -> Result<(), Fault> {
    value.numerator.prepay(budget)?;
    value.denominator.prepay(budget)?;
    let mut common = value
        .numerator
        .terms
        .keys()
        .next()
        .cloned()
        .unwrap_or_default();
    for term in value
        .numerator
        .terms
        .keys()
        .chain(value.denominator.terms.keys())
    {
        for (atom, exponent) in &mut common {
            budget.work(
                size(term.len(), AT)?
                    .checked_mul(budget.limits().name_bytes)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            *exponent = (*exponent).min(
                term.iter()
                    .find_map(|(key, power)| (key == &*atom).then_some(*power))
                    .unwrap_or(0),
            );
        }
    }
    common.retain(|(_, exponent)| *exponent != 0);
    for polynomial in [&mut value.numerator, &mut value.denominator] {
        let mut reduced = Polynomial {
            terms: BTreeMap::new(),
        };
        for (term, coefficient) in &polynomial.terms {
            let mut monomial = Vec::new();
            monomial
                .try_reserve_exact(term.len())
                .map_err(|_| Fault::allocation(AT))?;
            for (atom, exponent) in term {
                budget.work(
                    size(common.len(), AT)?
                        .checked_mul(budget.limits().name_bytes)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                    AT,
                )?;
                let divisor = common
                    .iter()
                    .find_map(|(key, power)| (key == atom).then_some(*power))
                    .unwrap_or(0);
                let exponent = exponent.checked_sub(divisor).ok_or_else(fragment)?;
                if exponent != 0 {
                    monomial.push((atom.clone(), exponent));
                }
            }
            reduced.accumulate(monomial, coefficient, budget)?;
        }
        *polynomial = reduced;
    }
    Ok(())
}

/// Materialize a finite ordered sum, omitting empty powers and unit coefficients.
fn materialize(
    cas: &mut Cas,
    context: LogicalId,
    sort: &Sort,
    value: &Polynomial,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    value.prepay(budget)?;
    let mut sum = None;
    for (monomial, coefficient) in &value.terms {
        let term = materialize_term(cas, context, sort, monomial, coefficient, budget)?;
        sum = Some(if let Some(previous) = sum {
            cas.generated(
                context,
                Operator::Add,
                sort.clone(),
                &[previous, term],
                budget,
            )?
        } else {
            term
        });
    }
    sum.map_or_else(
        || {
            cas.generated(
                context,
                Operator::Rational(Rational::zero()),
                sort.clone(),
                &[],
                budget,
            )
        },
        Ok,
    )
}

/// Produce one monomial without imposing a zero-power domain restriction.
fn materialize_term(
    cas: &mut Cas,
    context: LogicalId,
    sort: &Sort,
    monomial: &[(Atom, u64)],
    coefficient: &Rational,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut product = if monomial.is_empty() || *coefficient != Rational::one() {
        Some(cas.generated(
            context,
            Operator::Rational(coefficient.clone()),
            sort.clone(),
            &[],
            budget,
        )?)
    } else {
        None
    };
    for (atom, exponent) in monomial {
        let operator = match atom {
            Atom::Free(key) => Operator::Free(key.clone()),
            Atom::Constant(0) => Operator::Constant(Constant::Pi),
            Atom::Constant(_) => Operator::Constant(Constant::E),
        };
        let mut factor = cas.generated(context, operator, sort.clone(), &[], budget)?;
        if *exponent != 1 {
            let exponent = i64::try_from(*exponent).map_err(|_| fragment())?;
            factor = cas.generated(
                context,
                Operator::IntegerPower(exponent),
                sort.clone(),
                &[factor],
                budget,
            )?;
        }
        let combined = product.map_or_else(
            || Ok(factor),
            |previous| {
                cas.generated(
                    context,
                    Operator::Mul,
                    sort.clone(),
                    &[previous, factor],
                    budget,
                )
            },
        )?;
        product = Some(combined);
    }
    product.ok_or_else(fragment)
}
