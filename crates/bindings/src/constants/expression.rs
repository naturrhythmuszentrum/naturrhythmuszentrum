// SPDX-License-Identifier: AGPL-3.0-only
//! Original-node enclosure preparation in the existing constant export route.

use crate::UnitError;
use nrz_cas::enclosure::{
    CheckedEnclosure, EnclosureGoal, EnclosureOutcome, EnclosureReport, EnclosureRequest,
    RefinementPolicy,
};
use nrz_cas::limits::{Budget, BudgetKind, BudgetUsage};
use nrz_cas::syntax::Operator;
use nrz_cas::{Cas, Fault, Location, Rational};

/// Preparation failures preserve unsupported identity, resources and interruption separately.
#[derive(Debug, thiserror::Error)]
pub enum ExpressionEnclosureError {
    /// Original expression is outside the one admitted constant.
    #[error(transparent)]
    Unsupported(#[from] UnitError),
    /// Exact admission, arithmetic, storage or Work failed.
    #[error(transparent)]
    Exact(#[from] Fault),
    /// A charged preparation boundary observed interruption.
    #[error("constant enclosure preparation interrupted")]
    Interrupted,
    /// The finite refinement schedule did not certify the requested width.
    #[error("constant enclosure target not certifiable")]
    TargetNotCertifiable,
}

/// Checked preparation data; numerical authority additionally requires its frozen graph.
///
/// Fields cannot be supplied separately. Matrix calls `prepare` itself, then retains
/// this result with exclusive encapsulation of the producing graph. This is not a
/// numerical owner or a replacement for either existing coefficient export type.
#[derive(Debug)]
pub struct ExpressionEnclosure {
    /// Original request, checked endpoints, complete transcript and checker versions.
    checked: CheckedEnclosure,
    /// Actual bounded preparation policy, including the width goal.
    policy: RefinementPolicy,
    /// Exact lower and upper endpoint spellings, in that order.
    endpoints: [String; 2],
    /// Conservative logical storage of this retained result.
    bytes: u64,
}

/// All preparation and spelling charges use the caller's cumulative ledger.
const AT: Location = Location::new("expression enclosure export");

impl ExpressionEnclosure {
    /// Prepare the original `Mul(2, Log(2))` node, never an equal-spelled call.
    ///
    /// The receipt is reset and filled on every outcome; the supplied ledger also
    /// retains the caller's prefix. CAS independently checks each produced round.
    /// # Errors
    /// Refuses unsupported originals, insufficient exact resources, interruption
    /// and an unattained width without publishing a partial result.
    pub fn prepare(
        cas: &mut Cas,
        request: &EnclosureRequest,
        policy: RefinementPolicy,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
        work_used: &mut u64,
    ) -> Result<Self, ExpressionEnclosureError> {
        let start = budget.usage().work;
        *work_used = 0;
        let result = prepare(cas, request, policy, budget, interrupt);
        *work_used = budget.usage().work.saturating_sub(start);
        result
    }

    /// Actual checked judgment, including original live identities and proof closure.
    #[must_use]
    pub const fn judgment(&self) -> &CheckedEnclosure {
        &self.checked
    }

    /// Complete policy used to produce the retained judgment.
    #[must_use]
    pub const fn policy(&self) -> RefinementPolicy {
        self.policy
    }

    /// Ordered exact rational lower and upper spellings.
    #[must_use]
    pub fn endpoints(&self) -> [&str; 2] {
        [&self.endpoints[0], &self.endpoints[1]]
    }

    /// Conservative retained storage, excluding the separately retained graph.
    #[must_use]
    pub const fn owned_bytes(&self) -> u64 {
        self.bytes
    }
}

/// Original operators and ordered operands determine eligibility before refinement.
fn admit(
    cas: &Cas,
    request: &EnclosureRequest,
    b: &mut Budget,
) -> Result<(), ExpressionEnclosureError> {
    b.work(1, AT)?;
    let unsupported =
        || ExpressionEnclosureError::Unsupported(UnitError::UnsupportedExactConstantExport);
    if !request.coordinates.is_empty()
        || request.goal != EnclosureGoal::Width
        || !matches!(cas.expression(&request.target, b)?.0, Operator::Mul)
    {
        return Err(unsupported());
    }
    let operands = cas.operands(&request.target, b)?;
    let [factor, log] = operands.as_slice() else {
        return Err(unsupported());
    };
    let two = Rational::from_ratio(2, 1, b)?;
    if !matches!(cas.expression(factor, b)?.0, Operator::Rational(q) if q == &two)
        || !matches!(cas.expression(log, b)?.0, Operator::NaturalLog)
    {
        return Err(unsupported());
    }
    let arguments = cas.operands(log, b)?;
    let [argument] = arguments.as_slice() else {
        return Err(unsupported());
    };
    if !matches!(cas.expression(argument, b)?.0, Operator::Rational(q) if q == &two) {
        return Err(unsupported());
    }
    Ok(())
}

/// Move the actual checked result, preserving its complete key instead of reconstructing it.
fn prepare(
    cas: &mut Cas,
    request: &EnclosureRequest,
    policy: RefinementPolicy,
    b: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<ExpressionEnclosure, ExpressionEnclosureError> {
    admit(cas, request, b)?;
    if policy.version != 1 {
        return Err(Fault::invalid(nrz_cas::InvalidCode::Policy, AT).into());
    }
    let report = cas.enclose(request, policy, b, interrupt)?;
    let checked = completed(report)?;
    spell(cas, checked, policy, b)
}

/// A completed bound is usable only when its attempt stopped successfully at the width goal.
fn completed(report: EnclosureReport) -> Result<CheckedEnclosure, ExpressionEnclosureError> {
    if let Some(fault) = report.resource_fault {
        return Err(fault.into());
    }
    if report.outcome == EnclosureOutcome::Interrupted {
        return Err(ExpressionEnclosureError::Interrupted);
    }
    if !report.precision_met {
        return Err(ExpressionEnclosureError::TargetNotCertifiable);
    }
    report
        .checked
        .ok_or(ExpressionEnclosureError::TargetNotCertifiable)
}

/// Spell only the actual completed judgment while all of its live storage remains admitted.
fn spell(
    cas: &Cas,
    checked: CheckedEnclosure,
    policy: RefinementPolicy,
    b: &mut Budget,
) -> Result<ExpressionEnclosure, ExpressionEnclosureError> {
    let bytes = inventory(&checked, b)?.saturating_add(size_of::<ExpressionEnclosure>() as u64);
    cas.with_retained_graph(b, |b| {
        b.with_retained_payload(bytes, AT, |b| {
            b.check(BudgetKind::OutputBytes, bytes, AT)?;
            let lower = checked.interval().lower().exact_string(b)?;
            let upper = b.with_retained_payload(lower.capacity() as u64, AT, |b| {
                checked.interval().upper().exact_string(b)
            })?;
            let endpoints = [lower, upper];
            let bytes =
                bytes.saturating_add(endpoints.iter().map(|s| s.capacity() as u64).sum::<u64>());
            b.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            b.check(BudgetKind::OutputBytes, bytes, AT)?;
            Ok(ExpressionEnclosure {
                checked,
                policy,
                endpoints,
                bytes,
            })
        })
    })
}

#[cfg(test)]
mod tests;

/// Inventory the closed logarithm transcript before retaining it during spelling.
fn inventory(checked: &CheckedEnclosure, b: &mut Budget) -> Result<u64, Fault> {
    let proof = checked.certificate();
    b.work((proof.records.len() + proof.logarithms.len()) as u64, AT)?;
    let number = |q: &Rational| {
        let (n, d) = q.component_bits();
        (n.div_ceil(8) + d.div_ceil(8))
            .saturating_mul(4)
            .saturating_add(128)
    };
    let interval =
        |i: &nrz_cas::enclosure::Interval| number(i.lower()).saturating_add(number(i.upper()));
    let mut bytes = (proof.scope.capacity() as u64)
        .saturating_add(1024)
        .saturating_add(interval(checked.interval()));
    for record in &proof.records {
        bytes = bytes.saturating_add(interval(record));
    }
    for record in &proof.logarithms {
        bytes = bytes.saturating_add(128);
        for endpoint in [&record.lower, &record.upper] {
            bytes = bytes
                .saturating_add(number(&endpoint.mantissa))
                .saturating_add(number(&endpoint.argument));
            for series in [&endpoint.reduced, &endpoint.two] {
                bytes = bytes
                    .saturating_add(64)
                    .saturating_add(interval(&series.sum))
                    .saturating_add(interval(&series.power))
                    .saturating_add(interval(&series.tail));
            }
        }
    }
    Ok(bytes)
}
