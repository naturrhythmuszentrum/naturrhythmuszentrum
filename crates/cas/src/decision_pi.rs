// SPDX-License-Identifier: AGPL-3.0-only
//! Closed interval replay consumes the independently checked Machin theorem.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::budget::{Budget, BudgetKind};
use crate::decision::RationalEnclosure;
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{Constant, Operator};
use crate::number::Rational;
use crate::pi_bounds::PiBoundCertificate;

/// Closed pi replay has separate accounting from the unchanged rational-only route.
const AT: Location = Location::new("closed pi enclosure");

/// An unproved interval divisor cannot be used, even if the expression has a known sign.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Exact outward interval, with included endpoints and no floating intermediates.
#[derive(Clone)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar and lattice replay share exact outward interval arithmetic"
)]
pub(super) struct Interval {
    /// Included lower endpoint.
    pub(super) lower: Rational,
    /// Included upper endpoint.
    pub(super) upper: Rational,
}

impl Interval {
    /// Rational leaves are exact singleton intervals.
    fn point(value: &Rational) -> Self {
        Self {
            lower: value.clone(),
            upper: value.clone(),
        }
    }

    /// Both endpoint signs must exclude zero before taking a reciprocal or negative power.
    fn excludes_zero(&self) -> bool {
        self.upper.is_negative() || (!self.lower.is_negative() && !self.lower.is_zero())
    }

    /// Reciprocal reverses endpoints on either connected component of its domain.
    fn reciprocal(&self, budget: &mut Budget) -> Result<Self, Fault> {
        if !self.excludes_zero() {
            return Err(unsupported());
        }
        Ok(Self {
            lower: Rational::one().div(&self.upper, budget)?,
            upper: Rational::one().div(&self.lower, budget)?,
        })
    }

    /// The product of two closed intervals has extrema among its four endpoint products.
    pub(super) fn product(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let mut result = Self::point(&self.lower.mul(&other.lower, budget)?);
        for (left, right) in [
            (&self.lower, &other.upper),
            (&self.upper, &other.lower),
            (&self.upper, &other.upper),
        ] {
            let candidate = left.mul(right, budget)?;
            if candidate.compare(&result.lower, budget)? == Ordering::Less {
                result.lower.clone_from(&candidate);
            }
            if candidate.compare(&result.upper, budget)? == Ordering::Greater {
                result.upper = candidate;
            }
        }
        Ok(result)
    }

    /// Endpoint powers suffice except for an even positive power over a zero crossing.
    fn power(&self, exponent: i64, budget: &mut Budget) -> Result<Self, Fault> {
        if exponent <= 0 && !self.excludes_zero() {
            return Err(unsupported());
        }
        let mut lower = self.lower.pow(exponent, budget)?;
        let mut upper = self.upper.pow(exponent, budget)?;
        if lower.compare(&upper, budget)? == Ordering::Greater {
            std::mem::swap(&mut lower, &mut upper);
        }
        let even_positive = exponent > 0 && exponent % 2 == 0;
        if even_positive && !self.excludes_zero() {
            lower = Rational::zero();
        }
        Ok(Self { lower, upper })
    }

    /// Monotonicity of the nonnegative square root transports independently squared endpoints.
    fn square_root(&self, budget: &mut Budget) -> Result<Self, Fault> {
        if self.lower.is_negative() {
            return Err(unsupported());
        }
        let (lower, _) = self.lower.square_root_bounds(budget)?;
        let (_, upper) = self.upper.square_root_bounds(budget)?;
        Ok(Self { lower, upper })
    }

    /// Conservative retained map payload, excluding shared immutable graph syntax.
    fn bytes(&self) -> u64 {
        128_u64
            .saturating_add(number_bytes(&self.lower))
            .saturating_add(number_bytes(&self.upper))
    }
}

/// Validate original syntax, a finite pi expansion, and containment of the resulting box.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling closed replay checkers share this validation boundary"
)]
pub(super) fn validate(
    cas: &Cas,
    contract: &RationalEnclosure,
    expression: &Handle,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    budget.work(4, AT)?;
    cas.checked(expression)?;
    if expression != &contract.target {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    crate::decision_enclosure::identities(cas, contract)?;
    for endpoint in [&contract.lower, &contract.upper] {
        endpoint.check(budget, AT)?;
    }
    if contract.lower.compare(&contract.upper, budget)? == Ordering::Greater {
        return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
    }
    let bytes = 384_u64
        .saturating_add(certificate.validate(budget)?)
        .saturating_add(number_bytes(&contract.lower))
        .saturating_add(number_bytes(&contract.upper));
    let scratch = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(64)
        .saturating_add(1_024);
    budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
        // Existing admission checks every original operand before any cancellation.
        let checked = ExactConstant::from_expression(cas, expression, budget)?;
        let interval = enclose(&checked, certificate, budget)?;
        let above_lower = contract.lower.compare(&interval.lower, budget)? != Ordering::Greater;
        let below_upper = interval.upper.compare(&contract.upper, budget)? != Ordering::Greater;
        if above_lower && below_upper {
            Ok(())
        } else {
            Err(Fault::invalid(InvalidCode::DecisionBox, AT))
        }
    })?;
    Ok(bytes)
}

/// Reuse the closed grammar and postorder traversal of already checked normalized constants.
fn enclose(
    constant: &ExactConstant,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    if let Some(value) = constant.as_rational() {
        return Ok(Interval::point(value));
    }
    let (cas, root) = constant.graph().ok_or_else(unsupported)?;
    let root = cas.checked(root)?;
    let order = crate::exact_constant::graph::order(&cas.state, root, budget)?;
    let mut retained = crate::graph::size(order.len(), AT)?.saturating_mul(64);
    let mut facts = BTreeMap::new();
    for id in order {
        let term = cas.state.expression(id)?;
        let interval = budget.with_retained_payload(retained, AT, |budget| {
            evaluate(&term.operator, &term.operands, &facts, certificate, budget)
        })?;
        retained = retained.saturating_add(interval.bytes());
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.work(interval.bytes(), AT)?;
        facts.insert(id, interval);
    }
    facts.remove(&root).ok_or_else(unsupported)
}

/// All operations are outward rational reductions; no unknown named constant is approximated.
fn evaluate(
    operator: &Operator,
    operands: &[LogicalId],
    facts: &BTreeMap<LogicalId, Interval>,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let child = |position: usize| {
        operands
            .get(position)
            .and_then(|id| facts.get(id))
            .ok_or_else(unsupported)
    };
    match operator {
        Operator::Rational(value) => Ok(Interval::point(value)),
        Operator::Constant(Constant::Pi) => Ok(Interval {
            lower: certificate.lower.clone(),
            upper: certificate.upper.clone(),
        }),
        Operator::Neg => Ok(Interval {
            lower: child(0)?.upper.negated(budget)?,
            upper: child(0)?.lower.negated(budget)?,
        }),
        Operator::Add => Ok(Interval {
            lower: child(0)?.lower.add(&child(1)?.lower, budget)?,
            upper: child(0)?.upper.add(&child(1)?.upper, budget)?,
        }),
        Operator::Sub => Ok(Interval {
            lower: child(0)?.lower.sub(&child(1)?.upper, budget)?,
            upper: child(0)?.upper.sub(&child(1)?.lower, budget)?,
        }),
        Operator::Mul => child(0)?.product(child(1)?, budget),
        Operator::Div => child(0)?.product(&child(1)?.reciprocal(budget)?, budget),
        Operator::IntegerPower(exponent) => child(0)?.power(*exponent, budget),
        Operator::GeneralPower(crate::model::Branch::StrictReal) => {
            let half = Rational::from_ratio(1, 2, budget)?;
            let lower_half = child(1)?.lower == half;
            let upper_half = child(1)?.upper == half;
            if !(lower_half && upper_half) {
                return Err(unsupported());
            }
            child(0)?.square_root(budget)
        }
        _ => Err(unsupported()),
    }
}

/// Rational allocation overhead, without cloning private bigint components.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}
