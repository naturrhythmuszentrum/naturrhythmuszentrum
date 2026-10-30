// SPDX-License-Identifier: AGPL-3.0-only
//! Independent endpoint relations replay finite records without producer or planner calls.

use super::{AT, CheckedEnclosure, EnclosureCertificate, EnclosureRequest, Interval, boundary};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::error::{Fault, InvalidCode};
use crate::graph::Cas;
use crate::model::Operator;
use crate::number::Rational;
use std::cmp::Ordering;

/// Reconstruct each endpoint relation independently of producer arithmetic.
pub(super) fn check(
    cas: &Cas,
    request: &EnclosureRequest,
    certificate: &EnclosureCertificate,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<CheckedEnclosure, Fault> {
    super::interval::scratch(budget)?;
    if certificate.version != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    budget.check(
        BudgetKind::WitnessBytes,
        crate::graph::size(certificate.scope.len(), AT)?,
        AT,
    )?;
    budget.check(
        BudgetKind::ProofNodes,
        crate::graph::size(certificate.records.len(), AT)?,
        AT,
    )?;
    budget.work(crate::graph::size(certificate.scope.len(), AT)?, AT)?;
    if certificate.scope != super::scope::key(cas, request, budget)? {
        return Err(invalid());
    }
    let schedule = super::scope::schedule(cas, request, budget)?;
    if schedule.len() != certificate.records.len() {
        return Err(invalid());
    }
    let mut stack: Vec<usize> = Vec::new();
    stack
        .try_reserve(schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, id) in schedule.into_iter().enumerate() {
        boundary(budget, interrupt)?;
        let expression = cas.state.expression(id)?;
        let record = &certificate.records[index];
        record.lower().check(budget, AT)?;
        record.upper().check(budget, AT)?;
        let right = if expression.operands.is_empty() {
            None
        } else {
            Some(stack.pop().ok_or_else(invalid)?)
        };
        let left = if expression.operands.len() == 2 {
            Some(stack.pop().ok_or_else(invalid)?)
        } else {
            None
        };
        let (lower, upper, rounded) = match &expression.operator {
            Operator::Rational(value) => (
                value.add(&Rational::zero(), budget)?,
                value.add(&Rational::zero(), budget)?,
                false,
            ),
            Operator::Free(_) => {
                budget.work(crate::graph::size(request.coordinates.len(), AT)?, AT)?;
                let value = request
                    .coordinates
                    .iter()
                    .find(|v| v.coordinate.logical_id() == id)
                    .ok_or_else(invalid)?;
                (
                    value.interval.lower().add(&Rational::zero(), budget)?,
                    value.interval.upper().add(&Rational::zero(), budget)?,
                    false,
                )
            }
            Operator::Neg => {
                let value = &certificate.records[right.ok_or_else(invalid)?];
                (
                    value.upper().negated(budget)?,
                    value.lower().negated(budget)?,
                    true,
                )
            }
            operator => {
                let a = &certificate.records[left.ok_or_else(invalid)?];
                let b = &certificate.records[right.ok_or_else(invalid)?];
                let (lo, hi) = relation(operator, a, b, certificate.grid, budget)?;
                (lo, hi, true)
            }
        };
        let (lower, upper) = if rounded {
            grid(&lower, &upper, certificate.grid, budget)?
        } else {
            (lower, upper)
        };
        if record.lower().compare(&lower, budget)? != Ordering::Equal
            || record.upper().compare(&upper, budget)? != Ordering::Equal
        {
            return Err(invalid());
        }
        stack.push(index);
    }
    if stack.len() != 1 {
        return Err(invalid());
    }
    finish(request, certificate, budget)
}

/// Reserve the complete receipt before copying checked evidence for publication.
fn finish(
    request: &EnclosureRequest,
    certificate: &EnclosureCertificate,
    budget: &mut Budget,
) -> Result<CheckedEnclosure, Fault> {
    let last = certificate.records.last().ok_or_else(invalid)?;
    let (lower, upper) = grid(last.lower(), last.upper(), certificate.presentation, budget)?;
    let interval = Interval::new(lower, upper, budget)?;
    let bytes = certificate
        .records
        .iter()
        .fold(
            u64::try_from(certificate.scope.len())
                .unwrap_or(u64::MAX)
                .saturating_add(512),
            |sum, value| sum.saturating_add(super::interval_bytes(value)),
        )
        .saturating_add(super::request_bytes(request))
        .saturating_add(super::interval_bytes(&interval));
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
    budget.work(bytes, AT)?;
    Ok(CheckedEnclosure {
        request: request.clone(),
        certificate: certificate.clone(),
        interval,
    })
}

/// Replay a binary law from exact rational endpoint relations.
fn relation(
    operator: &Operator,
    a: &Interval,
    b: &Interval,
    precision: u32,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    match operator {
        Operator::Add => Ok((
            a.lower().add(b.lower(), budget)?,
            a.upper().add(b.upper(), budget)?,
        )),
        Operator::Sub => Ok((
            a.lower().sub(b.upper(), budget)?,
            a.upper().sub(b.lower(), budget)?,
        )),
        Operator::Mul => products(a.lower(), a.upper(), b.lower(), b.upper(), budget),
        Operator::Div => {
            if b.lower().is_zero()
                || b.upper().is_zero()
                || b.lower().is_negative() != b.upper().is_negative()
            {
                return Err(Fault::invalid(InvalidCode::ZeroDenominator, AT));
            }
            let lo = Rational::one().div(b.upper(), budget)?;
            let hi = Rational::one().div(b.lower(), budget)?;
            let (lo, hi) = grid(&lo, &hi, precision, budget)?;
            products(a.lower(), a.upper(), &lo, &hi, budget)
        }
        _ => Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
    }
}

/// Compute four endpoint products and their exact extrema.
fn products(
    a: &Rational,
    b: &Rational,
    c: &Rational,
    d: &Rational,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    let values = [
        a.mul(c, budget)?,
        a.mul(d, budget)?,
        b.mul(c, budget)?,
        b.mul(d, budget)?,
    ];
    let mut minimum = 0;
    let mut maximum = 0;
    for (i, value) in values.iter().enumerate().skip(1) {
        if value.compare(&values[minimum], budget)? == Ordering::Less {
            minimum = i;
        }
        if value.compare(&values[maximum], budget)? == Ordering::Greater {
            maximum = i;
        }
    }
    Ok((
        values[minimum].add(&Rational::zero(), budget)?,
        values[maximum].add(&Rational::zero(), budget)?,
    ))
}

/// Replay outward rounding using exact division and signed floor.
fn grid(
    lower: &Rational,
    upper: &Rational,
    q: u32,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    budget.check(BudgetKind::ComponentBits, u64::from(q) + 1, AT)?;
    let unit = Rational::from_ratio(2, 1, budget)?.pow(-i64::from(q), budget)?;
    let lo = lower
        .div(&unit, budget)?
        .floor(budget)?
        .mul(&unit, budget)?;
    let hi = upper
        .div(&unit, budget)?
        .negated(budget)?
        .floor(budget)?
        .negated(budget)?
        .mul(&unit, budget)?;
    Ok((lo, hi))
}

/// Refuse an endpoint transcript that does not match its reconstructed law.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
