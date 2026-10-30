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
    let schedule = prepare(cas, request, certificate, budget)?;
    let mut replay = Replay {
        certificate,
        logs: certificate.logarithms.iter(),
        exps: certificate.exponentials.iter(),
        powers: certificate.powers.iter(),
        derivatives: certificate.derivatives.iter(),
        log_two: Vec::new(),
        roots: false,
        sigmoids: false,
        tangents: false,
    };
    let mut stack: Vec<usize> = Vec::new();
    stack
        .try_reserve(schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, id) in schedule.iter().copied().enumerate() {
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
        replay.node(
            cas,
            request,
            (id, index, &schedule),
            (left, right),
            budget,
            interrupt,
        )?;
        stack.push(index);
    }
    if stack.len() != 1
        || replay.logs.next().is_some()
        || replay.exps.next().is_some()
        || replay.powers.next().is_some()
        || replay.derivatives.next().is_some()
        || (certificate.law == super::EnclosureLaw::SmoothDerivative
            && certificate.derivatives.is_empty())
    {
        return Err(invalid());
    }
    finish(
        request,
        certificate,
        (replay.roots, replay.sigmoids, replay.tangents),
        budget,
    )
}

/// One independent transcript traversal owns its component cursors and checked log-two reuse.
struct Replay<'a> {
    /// Proposed records joined to the original source schedule.
    certificate: &'a EnclosureCertificate,
    /// Unconsumed logarithm components.
    logs: std::slice::Iter<'a, super::LogEvidence>,
    /// Unconsumed exponential components.
    exps: std::slice::Iter<'a, super::ExpEvidence>,
    /// Unconsumed positive-power routes.
    powers: std::slice::Iter<'a, super::PowerEvidence>,
    /// Unconsumed original derivative subtrees.
    derivatives: std::slice::Iter<'a, super::DerivativeEvidence>,
    /// Already checked one-third series for this transcript only.
    log_two: Vec<(&'a super::LogSeries, Interval)>,
    /// Encountered root premises.
    roots: bool,
    /// Encountered logistic premises.
    sigmoids: bool,
    /// Encountered hyperbolic premises.
    tangents: bool,
}

impl Replay<'_> {
    /// Dispatch only the original operator and independently checked earlier endpoints.
    fn node(
        &mut self,
        cas: &Cas,
        request: &EnclosureRequest,
        position: (crate::graph::LogicalId, usize, &[crate::graph::LogicalId]),
        inputs: (Option<usize>, Option<usize>),
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<(), Fault> {
        let (id, index, schedule) = position;
        let (left, right) = inputs;
        let expression = cas.state.expression(id)?;
        let record = &self.certificate.records[index];
        let (lower, upper, rounded) = match &expression.operator {
            Operator::Rational(value) => (
                value.add(&Rational::zero(), budget)?,
                value.add(&Rational::zero(), budget)?,
                false,
            ),
            Operator::Free(_) => coordinate(request, id, budget)?,
            Operator::Neg => negated(
                &self.certificate.records[right.ok_or_else(invalid)?],
                budget,
            )?,
            Operator::NaturalLog | Operator::Exponential | Operator::Sigmoid | Operator::Tanh => {
                return self.analytic(
                    &expression.operator,
                    right.ok_or_else(invalid)?,
                    index,
                    budget,
                    interrupt,
                );
            }
            Operator::Derivative(_) | Operator::GeneralizedDerivative(_) => {
                return self.derivative(cas, expression, (schedule, index), budget, interrupt);
            }
            Operator::PositivePower => {
                return self.power(
                    cas,
                    expression,
                    index,
                    left.ok_or_else(invalid)?,
                    budget,
                    interrupt,
                );
            }
            Operator::ClampFloor => {
                let (lo, hi) = super::clamp::check_value(
                    &cas.state,
                    expression,
                    &self.certificate.records[left.ok_or_else(invalid)?],
                    &self.certificate.records[right.ok_or_else(invalid)?],
                    self.certificate.grid,
                    budget,
                )?;
                return endpoint_match(record, (lo, hi), false, self.certificate.grid, budget);
            }
            Operator::GeneralPower(crate::model::Branch::StrictReal) => {
                let exponent = expression.operands.get(1).ok_or_else(invalid)?;
                super::roots::exponent(&cas.state.expression(*exponent)?.operator)?;
                super::roots::check(
                    &self.certificate.records[left.ok_or_else(invalid)?],
                    record,
                    self.certificate.grid,
                    budget,
                )?;
                self.roots = true;
                return Ok(());
            }
            operator => {
                let (lo, hi) = relation(
                    operator,
                    &self.certificate.records[left.ok_or_else(invalid)?],
                    &self.certificate.records[right.ok_or_else(invalid)?],
                    self.certificate.grid,
                    budget,
                )?;
                (lo, hi, true)
            }
        };
        endpoint_match(
            record,
            (lower, upper),
            rounded,
            self.certificate.grid,
            budget,
        )
    }

    /// Check each analytic record at its exact original index.
    fn analytic(
        &mut self,
        operator: &Operator,
        input: usize,
        index: usize,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<(), Fault> {
        let value = &self.certificate.records[input];
        let record = &self.certificate.records[index];
        if matches!(operator, Operator::NaturalLog) {
            let evidence = self.logs.next().ok_or_else(invalid)?;
            if usize::try_from(evidence.record).ok() != Some(index) {
                return Err(invalid());
            }
            return super::log::check(
                value,
                evidence,
                record,
                self.certificate.grid,
                &mut self.log_two,
                budget,
                interrupt,
            );
        }
        let evidence = self.exps.next().ok_or_else(invalid)?;
        if usize::try_from(evidence.record).ok() != Some(index) {
            return Err(invalid());
        }
        self.sigmoids |= matches!(operator, Operator::Sigmoid);
        self.tangents |= matches!(operator, Operator::Tanh);
        let check = match operator {
            Operator::Tanh => super::saturating::check_tanh,
            Operator::Sigmoid => super::saturating::check,
            _ => super::exp::check,
        };
        check(
            value,
            evidence,
            record,
            self.certificate.grid,
            budget,
            interrupt,
        )
    }

    /// Reconstruct the power route from its original literal exponent, not grammar flags.
    fn power(
        &mut self,
        cas: &Cas,
        expression: &crate::graph::Expression,
        index: usize,
        input: usize,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<(), Fault> {
        if !matches!(
            self.certificate.law,
            super::EnclosureLaw::PositivePower
                | super::EnclosureLaw::SmoothDerivative
                | super::EnclosureLaw::ClampGeneralized
        ) {
            return Err(invalid());
        }
        let evidence = self.powers.next().ok_or_else(invalid)?;
        if usize::try_from(evidence.record).ok() != Some(index) {
            return Err(invalid());
        }
        let Operator::Rational(exponent) = &cas.state.expression(expression.operands[1])?.operator
        else {
            return Err(invalid());
        };
        let (log, exp) = if evidence.route == super::PowerRoute::LogExp {
            (
                Some(self.logs.next().ok_or_else(invalid)?),
                Some(self.exps.next().ok_or_else(invalid)?),
            )
        } else {
            (None, None)
        };
        super::power::check(
            (&self.certificate.records[input], exponent),
            evidence,
            &mut (log, exp, &mut self.log_two),
            &self.certificate.records[index],
            self.certificate.grid,
            budget,
            interrupt,
        )?;
        self.roots |= !evidence.roots.is_empty();
        Ok(())
    }
    /// Authenticate both the symbolic coordinate and the complete original subtree join.
    fn derivative(
        &mut self,
        cas: &Cas,
        expression: &crate::graph::Expression,
        position: (&[crate::graph::LogicalId], usize),
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<(), Fault> {
        if !matches!(
            self.certificate.law,
            super::EnclosureLaw::SmoothDerivative | super::EnclosureLaw::ClampGeneralized
        ) {
            return Err(invalid());
        }
        let (Operator::Derivative(coordinate) | Operator::GeneralizedDerivative(coordinate)) =
            &expression.operator
        else {
            return Err(invalid());
        };
        let (schedule, index) = position;
        if matches!(expression.operator, Operator::GeneralizedDerivative(_))
            && index + 1 != self.certificate.records.len()
        {
            return Err(invalid());
        }
        let count = usize::try_from(cas.state.expression(expression.operands[0])?.unfolded)
            .map_err(|_| invalid())?;
        let first = index.checked_sub(count).ok_or_else(invalid)?;
        let evidence = self.derivatives.next().ok_or_else(invalid)?;
        if usize::try_from(evidence.record).ok() != Some(index)
            || usize::try_from(evidence.first).ok() != Some(first)
            || &evidence.coordinate != coordinate
        {
            return Err(invalid());
        }
        let source = super::derivative::Segment {
            cas,
            schedule: &schedule[first..index],
            values: &self.certificate.records[first..index],
            generalized: matches!(expression.operator, Operator::GeneralizedDerivative(_)),
        };
        super::derivative::check(
            source,
            evidence,
            &self.certificate.records[index],
            self.certificate.grid,
            budget,
            interrupt,
        )
    }
}

/// Bind the registered family and version to the original expression's derived grammar.
fn schema(
    cas: &Cas,
    request: &EnclosureRequest,
    certificate: &EnclosureCertificate,
) -> Result<(), Fault> {
    let expected_law = if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_clamp
        != 0
    {
        super::EnclosureLaw::ClampGeneralized
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_derivative
        != 0
    {
        super::EnclosureLaw::SmoothDerivative
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_power
        .present
    {
        super::EnclosureLaw::PositivePower
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_saturation
        == 2
    {
        super::EnclosureLaw::TanhCompose
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_saturation
        == 1
    {
        super::EnclosureLaw::SigmoidCompose
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_exp
        .present
    {
        super::EnclosureLaw::ExpTaylor
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_logs
    {
        super::EnclosureLaw::LogAtanh
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_roots
    {
        super::EnclosureLaw::RootGrid
    } else {
        super::EnclosureLaw::RationalInclusion
    };
    if certificate.law != expected_law {
        return Err(invalid());
    }
    if certificate.version != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    Ok(())
}

/// Reserve the complete receipt before copying checked evidence for publication.
fn finish(
    request: &EnclosureRequest,
    certificate: &EnclosureCertificate,
    families: (bool, bool, bool),
    budget: &mut Budget,
) -> Result<CheckedEnclosure, Fault> {
    let last = certificate.records.last().ok_or_else(invalid)?;
    let (lower, upper) = grid(last.lower(), last.upper(), certificate.presentation, budget)?;
    let interval = Interval::new(lower, upper, budget)?;
    let bytes = super::derivative::bytes(&certificate.derivatives)
        .saturating_add(super::power::bytes(&certificate.powers))
        .saturating_add(super::exp::bytes(&certificate.exponentials))
        .saturating_add(super::log::bytes(&certificate.logarithms))
        .saturating_add(
            certificate
                .records
                .iter()
                .fold(
                    u64::try_from(certificate.scope.len())
                        .unwrap_or(u64::MAX)
                        .saturating_add(512),
                    |sum, value| sum.saturating_add(super::interval_bytes(value)),
                )
                .saturating_add(super::request_bytes(request))
                .saturating_add(super::interval_bytes(&interval)),
        );
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
    budget.work(bytes, AT)?;
    Ok(CheckedEnclosure {
        roots: families.0,
        sigmoids: families.1,
        tangents: families.2,
        request: request.clone(),
        certificate: certificate.clone(),
        interval,
    })
}

/// Replay a binary law from exact rational endpoint relations.
pub(super) fn relation(
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
pub(super) fn grid(
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

/// Validate the original join and reserve the independent replay schedule.
fn prepare(
    cas: &Cas,
    request: &EnclosureRequest,
    certificate: &EnclosureCertificate,
    budget: &mut Budget,
) -> Result<Vec<crate::graph::LogicalId>, Fault> {
    super::interval::scratch(budget)?;
    schema(cas, request, certificate)?;
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
    Ok(schedule)
}

/// Authenticate the complete original coordinate and copy its checked endpoints.
fn coordinate(
    request: &EnclosureRequest,
    id: crate::graph::LogicalId,
    budget: &mut Budget,
) -> Result<(Rational, Rational, bool), Fault> {
    budget.work(crate::graph::size(request.coordinates.len(), AT)?, AT)?;
    let value = request
        .coordinates
        .iter()
        .find(|v| v.coordinate.logical_id() == id)
        .ok_or_else(invalid)?;
    Ok((
        value.interval.lower().add(&Rational::zero(), budget)?,
        value.interval.upper().add(&Rational::zero(), budget)?,
        false,
    ))
}

/// Negation reverses and negates exact endpoints before outward gridding.
fn negated(value: &Interval, budget: &mut Budget) -> Result<(Rational, Rational, bool), Fault> {
    Ok((
        value.upper().negated(budget)?,
        value.lower().negated(budget)?,
        true,
    ))
}

/// Compare the exact reconstructed relation after its required outward rounding.
fn endpoint_match(
    record: &Interval,
    bounds: (Rational, Rational),
    rounded: bool,
    q: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (lower, upper) = bounds;
    let (lower, upper) = if rounded {
        grid(&lower, &upper, q, budget)?
    } else {
        (lower, upper)
    };
    if record.lower().compare(&lower, budget)? != Ordering::Equal
        || record.upper().compare(&upper, budget)? != Ordering::Equal
    {
        return Err(invalid());
    }
    Ok(())
}
