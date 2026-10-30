// SPDX-License-Identifier: AGPL-3.0-only
//! Closed scalar record replay keeps source coverage separate from endpoint arithmetic.

#[expect(
    clippy::redundant_pub_crate,
    reason = "the store and enclosure dispatcher share checked record retention"
)]
pub(crate) mod cache;
mod input;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the public decision facade re-exports the pure certificate model"
)]
pub(super) mod model;
mod relations;
mod root;
mod wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling decision wire dispatcher shares the scalar origin grammar"
)]
pub(super) use wire::{decode, encode};

use crate::budget::{Budget, BudgetKind};
use crate::decision::RationalEnclosure;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, size};
use crate::number::Rational;
use model::{ScalarKernelCertificate, ScalarKernelLaw, ScalarKernelRecord};

/// Original target, assurance scope and admitted family are checked at this boundary.
const AT: Location = Location::new("scalar kernel coverage");

/// A source-free certificate cannot stand for an arbitrary source family.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Authenticate scope, replay every relation, and match the complete original target.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling enclosure checker dispatches closed scalar certificates"
)]
pub(super) fn validate(
    cas: &Cas,
    contract: &RationalEnclosure,
    certificate: &ScalarKernelCertificate,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    budget.work(4, AT)?;
    if !matches!(certificate.version, 1 | 2) {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    for handle in [
        &certificate.expression,
        &certificate.context,
        &certificate.input_family,
    ] {
        cas.checked(handle)?;
    }
    let same_target = certificate.expression == contract.target;
    let same_context = certificate.context == contract.context;
    let same_family = certificate.input_family == contract.input_family;
    if !(same_target && same_context && same_family) {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    crate::decision_enclosure::identities(cas, contract)?;
    if certificate.records.is_empty() {
        return Err(unsupported());
    }
    let count = size(certificate.records.len(), AT)?;
    budget.check(BudgetKind::ConditionAtoms, count, AT)?;
    budget.work(count, AT)?;
    let bytes = certificate
        .records
        .iter()
        .fold(512_u64, |total, record| {
            total.saturating_add(record_bytes(record))
        })
        .saturating_add(number_bytes(&contract.lower))
        .saturating_add(number_bytes(&contract.upper));
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        let original =
            ExactConstant::from_expression(cas, &contract.target, budget).map_err(fragment)?;
        let retained = original.retained_payload_bytes()?;
        budget.with_retained_payload(retained, AT, |budget| {
            checked_records(cas, contract, certificate, &original, budget)
        })
    })?;
    Ok(bytes)
}

/// Canonical encoding and lookup use the existing complete-record copying allowance.
fn checked_records(
    cas: &Cas,
    contract: &RationalEnclosure,
    certificate: &ScalarKernelCertificate,
    original: &ExactConstant,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let before = budget.usage().work;
    let key = cache::key(cas, certificate, budget)?;
    let cached = cache::lookup(cas, &key, budget)?;
    let copying = certificate.records.iter().fold(0_u64, |total, record| {
        total.saturating_add(record_bytes(record))
    });
    budget.work(copying.saturating_sub(budget.usage().work - before), AT)?;
    let values = budget.with_retained_payload(size(key.len(), AT)?, AT, |budget| {
        replay(
            cas,
            contract,
            certificate,
            original,
            cached.as_deref().map(Vec::as_slice),
            budget,
        )
    })?;
    if cached.is_none() {
        cache::insert(cas, key, values, budget)?;
    }
    Ok(())
}

/// Retain every earlier exact value and full interval while checking the final original value.
fn replay(
    cas: &Cas,
    contract: &RationalEnclosure,
    certificate: &ScalarKernelCertificate,
    original: &ExactConstant,
    cached: Option<&[ExactConstant]>,
    budget: &mut Budget,
) -> Result<Vec<ExactConstant>, Fault> {
    let mut values = Vec::new();
    let mut retained = size(certificate.records.len(), AT)?.saturating_mul(64);
    budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
    budget.work(retained, AT)?;
    values
        .try_reserve_exact(certificate.records.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, record) in certificate.records.iter().enumerate() {
        let value = budget.with_retained_payload(retained, AT, |budget| {
            if certificate.version == 1 && matches!(record.law, ScalarKernelLaw::Input(_)) {
                return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
            }
            relations::check(cas, record, &certificate.records[..index], budget)?;
            if let Some(cached) = cached {
                return cached.get(index).cloned().ok_or_else(unsupported);
            }
            let expected = (index + 1 == certificate.records.len()).then_some(original);
            exact(&record.law, &values, expected, budget)
        })?;
        retained = retained.saturating_add(value.retained_payload_bytes()?);
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        values.push(value);
    }
    budget.work(retained, AT)?;
    let value = values.last().ok_or_else(unsupported)?;
    if value != original {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    let last = certificate.records.last().ok_or_else(unsupported)?;
    let same_lower = last.lower == contract.lower;
    let same_upper = last.upper == contract.upper;
    if !(same_lower && same_upper) {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    Ok(values)
}

/// Reconstruct exact mathematical meaning using the existing independent constant normalizer.
fn exact(
    law: &ScalarKernelLaw,
    values: &[ExactConstant],
    original: Option<&ExactConstant>,
    budget: &mut Budget,
) -> Result<ExactConstant, Fault> {
    let child = |index: u32| {
        values
            .get(usize::try_from(index).map_err(|_| unsupported())?)
            .ok_or_else(unsupported)
    };
    match law {
        ScalarKernelLaw::Rational(value) => Ok(value.clone().into()),
        ScalarKernelLaw::Input(value) => input::exact(value, budget),
        ScalarKernelLaw::Sum { left, right } => child(*left)?.add(child(*right)?, budget),
        ScalarKernelLaw::Product { left, right } => child(*left)?.mul(child(*right)?, budget),
        ScalarKernelLaw::Reciprocal { input } => ExactConstant::one().div(child(*input)?, budget),
        ScalarKernelLaw::SquareRoot { input } => {
            let value = child(*input)?;
            if let Some(original) = original
                && root::matches(original, value, budget)?
            {
                return Ok(original.clone());
            }
            value.positive().ok_or_else(unsupported)?.sqrt(budget)
        }
    }
}

/// Open targets cannot inherit a nominal constant certificate's coverage.
fn fragment(fault: Fault) -> Fault {
    if fault.kind == FaultKind::Invalid(InvalidCode::UnsupportedFragment) {
        unsupported()
    } else {
        fault
    }
}

/// Conservative complete logical record ownership before copying or exact replay.
fn record_bytes(record: &ScalarKernelRecord) -> u64 {
    let value = match &record.law {
        ScalarKernelLaw::Rational(value) => number_bytes(value),
        ScalarKernelLaw::Input(value) => input::bytes(value),
        ScalarKernelLaw::Sum { .. }
        | ScalarKernelLaw::Product { .. }
        | ScalarKernelLaw::Reciprocal { .. }
        | ScalarKernelLaw::SquareRoot { .. } => 0,
    };
    128_u64
        .saturating_add(value)
        .saturating_add(number_bytes(&record.lower))
        .saturating_add(number_bytes(&record.upper))
}

/// Rational component ownership, including conservative bigint allocation overhead.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}
