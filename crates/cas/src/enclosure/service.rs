// SPDX-License-Identifier: AGPL-3.0-only
//! Completed rounds publish atomically; stopping never invents a mathematical verdict.

use super::{
    AT, CheckedEnclosure, EnclosureCertificate, EnclosureGoal, EnclosureOutcome, EnclosureReport,
    EnclosureRequest, RefinementPolicy, UndecidedReason, boundary,
};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, VertexKind};
use crate::number::Rational;
use std::cmp::Ordering;

impl Cas {
    /// Enclose the supported original rational expression, retaining only checked rounds.
    /// The callback is observed after charged boundaries; `true` interrupts without a verdict.
    /// # Errors
    /// Refuses malformed scope, invalid domains or inconsistent evidence. Resource stopping
    /// is reported separately and preserves every previously committed round.
    pub fn enclose(
        &mut self,
        request: &EnclosureRequest,
        policy: RefinementPolicy,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<EnclosureReport, Fault> {
        let start = budget.usage().work;
        let previous = budget.restrict_storage(&self.limits);
        let result = evaluate(self, request, policy, budget, interrupt, start);
        budget.restore_limits(previous);
        result
    }

    /// Independently replay supplied evidence against the complete expected original request.
    /// Successful checking can publish an anonymous graph judgment and needs mutable access.
    /// # Errors
    /// Refuses malformed, mismatched or invalid certificates; none is downgraded to indecision.
    pub fn check_enclosure(
        &mut self,
        request: &EnclosureRequest,
        policy: RefinementPolicy,
        certificate: &EnclosureCertificate,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    ) -> Result<EnclosureReport, Fault> {
        let start = budget.usage().work;
        let previous = budget.restrict_storage(&self.limits);
        let result = replay(self, request, policy, certificate, budget, interrupt, start);
        budget.restore_limits(previous);
        result
    }
}

/// Refine checked rounds transactionally, retaining completed evidence on stopping.
fn evaluate(
    cas: &mut Cas,
    request: &EnclosureRequest,
    policy: RefinementPolicy,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    start: u64,
) -> Result<EnclosureReport, Fault> {
    let mut report = empty(start, budget);
    let initial = super::scope::preflight(cas, request, policy, budget);
    let Some(mut grid) = preflight(initial, &mut report, start, budget)? else {
        return Ok(report);
    };
    for _ in 0..policy.rounds {
        if grid > policy.grid_ceiling {
            break;
        }
        let attempt = cas.transaction(budget, |cas, budget| {
            boundary(budget, interrupt)?;
            let scope = super::scope::key(cas, request, budget)?;
            let prefix = prefix(&scope, grid, policy.precision + 2, budget)?;
            if let Some(checked) = cas.retained_enclosure(&prefix, budget)? {
                let bytes = super::footprint(checked);
                budget.check(BudgetKind::OutputBytes, bytes, AT)?;
                budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
                for record in &checked.certificate.records {
                    record.lower().check(budget, AT)?;
                    record.upper().check(budget, AT)?;
                }
                budget.work(bytes, AT)?;
                let checked = checked.clone();
                let decision = decide(&checked, policy.precision, budget)?;
                return Ok((checked, decision));
            }
            let reserved = super::receipt_reservation(cas, request, scope.len(), budget)?;
            budget.check(BudgetKind::OutputBytes, reserved, AT)?;
            let previous = budget.reserve_enclosure_checking(policy.checking_work);
            let retained = cas
                .retained_judgment_bytes()
                .saturating_add(super::request_bytes(request));
            let generated =
                budget.with_retained_payload(retained.saturating_add(reserved), AT, |budget| {
                    super::producer::generate(
                        cas,
                        request,
                        scope,
                        (grid, policy.precision + 2),
                        budget,
                        interrupt,
                    )
                });
            budget.restore_limits(previous);
            let certificate = generated?;
            let checked = budget.with_retained_payload(
                retained.saturating_add(super::certificate_bytes(&certificate)),
                AT,
                |budget| super::checker::check(cas, request, &certificate, budget, interrupt),
            )?;
            if let Some(previous) = &report.checked {
                let intersection = previous.interval.intersection(&checked.interval, budget)?;
                if intersection != checked.interval {
                    return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
                }
            }
            let decision = decide(&checked, policy.precision, budget)?;
            boundary(budget, interrupt)?;
            let key = retention_key(&prefix, &certificate, budget)?;
            budget.work(super::footprint(&checked), AT)?;
            cas.retain_enclosure(key, checked.clone(), budget)?;
            Ok((checked, decision))
        });
        match attempt {
            Ok((checked, (outcome, met))) => {
                report.checked = Some(checked);
                report.completed_rounds += 1;
                report.outcome = outcome;
                report.precision_met = met;
                if met
                    || matches!(
                        outcome,
                        EnclosureOutcome::Proved | EnclosureOutcome::Refuted
                    )
                {
                    break;
                }
            }
            Err(fault) => {
                stopping(fault, &mut report)?;
                break;
            }
        }
        let Some(next) = grid
            .checked_sub(policy.precision)
            .and_then(|guard| guard.checked_mul(2))
            .and_then(|guard| policy.precision.checked_add(guard))
        else {
            break;
        };
        grid = next;
    }
    report.work = budget.usage().work.saturating_sub(start);
    report.usage = budget.usage();
    Ok(report)
}

/// Check and retain an externally supplied transcript under the expected request.
fn replay(
    cas: &mut Cas,
    request: &EnclosureRequest,
    policy: RefinementPolicy,
    certificate: &EnclosureCertificate,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    start: u64,
) -> Result<EnclosureReport, Fault> {
    let mut report = empty(start, budget);
    let initial = super::scope::preflight(cas, request, policy, budget);
    if preflight(initial, &mut report, start, budget)?.is_none() {
        return Ok(report);
    }
    if certificate.grid > policy.grid_ceiling || certificate.presentation != policy.precision + 2 {
        return Err(Fault::invalid(InvalidCode::Policy, AT));
    }
    let attempt = cas.transaction(budget, |cas, budget| {
        boundary(budget, interrupt)?;
        let retained = cas
            .retained_judgment_bytes()
            .saturating_add(super::request_bytes(request));
        let checked = budget.with_retained_payload(
            retained.saturating_add(super::certificate_bytes(certificate)),
            AT,
            |budget| super::checker::check(cas, request, certificate, budget, interrupt),
        )?;
        let decision = decide(&checked, policy.precision, budget)?;
        let prefix = prefix(
            &certificate.scope,
            certificate.grid,
            certificate.presentation,
            budget,
        )?;
        let key = retention_key(&prefix, certificate, budget)?;
        boundary(budget, interrupt)?;
        budget.work(super::footprint(&checked), AT)?;
        cas.retain_enclosure(key, checked.clone(), budget)?;
        Ok((checked, decision))
    });
    match attempt {
        Ok((checked, (outcome, met))) => {
            report.checked = Some(checked);
            report.completed_rounds = 1;
            report.outcome = outcome;
            report.precision_met = met;
        }
        Err(fault) => stopping(fault, &mut report)?,
    }
    report.work = budget.usage().work.saturating_sub(start);
    report.usage = budget.usage();
    Ok(report)
}

/// Bind retained evidence to its schema, scope and both grids.
pub(super) fn prefix(
    scope: &[u8],
    grid: u32,
    presentation: u32,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = crate::encoding::Encoder::witness(budget, AT);
    out.byte(VertexKind::AnonymousLemma.tag())?;
    out.byte(3)?;
    out.unsigned(1)?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.unsigned(u64::from(grid))?;
    out.unsigned(u64::from(presentation))?;
    Ok(out.finish())
}

/// Include the complete canonical transcript in the retained statement key.
pub(super) fn retention_key(
    prefix: &[u8],
    certificate: &EnclosureCertificate,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let bytes = certificate.to_bytes(budget)?;
    let mut out = crate::encoding::Encoder::witness(budget, AT);
    out.raw(prefix)?;
    out.count(bytes.len())?;
    out.raw(&bytes)?;
    Ok(out.finish())
}

/// Translate unsupported and resource outcomes without inventing a verdict.
fn preflight(
    result: Result<Option<u32>, Fault>,
    report: &mut EnclosureReport,
    start: u64,
    budget: &Budget,
) -> Result<Option<u32>, Fault> {
    match result {
        Ok(None) => report.outcome = EnclosureOutcome::Unsupported,
        Ok(Some(grid)) => return Ok(Some(grid)),
        Err(fault) => stopping(fault, report)?,
    }
    report.work = budget.usage().work.saturating_sub(start);
    report.usage = budget.usage();
    Ok(None)
}

/// Initialize a report before any checked bound exists.
const fn empty(start: u64, budget: &Budget) -> EnclosureReport {
    EnclosureReport {
        outcome: EnclosureOutcome::Undecided(UndecidedReason::WidthLimit),
        checked: None,
        precision_met: false,
        completed_rounds: 0,
        work: budget.usage().work.saturating_sub(start),
        usage: budget.usage(),
        resource_fault: None,
    }
}

/// Keep invalid evidence distinct from interruption and resource exhaustion.
fn stopping(fault: Fault, report: &mut EnclosureReport) -> Result<(), Fault> {
    if fault.location == super::INTERRUPTED {
        report.outcome = EnclosureOutcome::Interrupted;
    } else if matches!(fault.kind, FaultKind::Exhausted { .. }) {
        report.resource_fault = Some(fault);
        report.outcome = if report.checked.is_some() {
            EnclosureOutcome::Undecided(UndecidedReason::WorkExhausted)
        } else {
            EnclosureOutcome::Exhausted
        };
    } else {
        return Err(fault);
    }
    Ok(())
}

/// Classify only a checked interval against the original goal.
fn decide(
    checked: &CheckedEnclosure,
    precision: u32,
    budget: &mut Budget,
) -> Result<(EnclosureOutcome, bool), Fault> {
    let epsilon = Rational::from_ratio(2, 1, budget)?.pow(-i64::from(precision), budget)?;
    let met = checked.interval.width(budget)?.compare(&epsilon, budget)? != Ordering::Greater;
    let bounds = &checked.interval;
    let (proved, refuted) = match &checked.request.goal {
        EnclosureGoal::Width => {
            return Ok((
                if met {
                    EnclosureOutcome::Bound
                } else {
                    EnclosureOutcome::Undecided(UndecidedReason::WidthLimit)
                },
                met,
            ));
        }
        EnclosureGoal::AtMost(threshold) => (
            bounds.upper().compare(threshold, budget)? != Ordering::Greater,
            bounds.lower().compare(threshold, budget)? == Ordering::Greater,
        ),
        EnclosureGoal::LessThan(threshold) => (
            bounds.upper().compare(threshold, budget)? == Ordering::Less,
            bounds.lower().compare(threshold, budget)? != Ordering::Less,
        ),
        EnclosureGoal::Equal(threshold) => (
            bounds.lower().compare(threshold, budget)? == Ordering::Equal
                && bounds.upper().compare(threshold, budget)? == Ordering::Equal,
            bounds.upper().compare(threshold, budget)? == Ordering::Less
                || bounds.lower().compare(threshold, budget)? == Ordering::Greater,
        ),
    };
    Ok((
        if proved {
            EnclosureOutcome::Proved
        } else if refuted {
            EnclosureOutcome::Refuted
        } else {
            EnclosureOutcome::Undecided(UndecidedReason::Straddle)
        },
        met,
    ))
}
