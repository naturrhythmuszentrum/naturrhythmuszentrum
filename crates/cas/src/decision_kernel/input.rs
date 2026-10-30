// SPDX-License-Identifier: AGPL-3.0-only
//! Full native input sets and their independent closed nominal sources.

use super::{
    model::{ScalarKernelInput, ScalarKernelInputValue, ScalarKernelRecord},
    number_bytes, relations,
};
use crate::budget::Budget;
use crate::{ExactConstant, Fault, InvalidCode, Location, syntax::Constant};
use std::cmp::Ordering;

/// A nominal singleton cannot replace the supplied complete total-radius set.
const AT: Location = Location::new("scalar kernel input coverage");

/// Check both full-set endpoint relations and the separately proved nominal source.
pub(super) fn check(
    cas: &crate::graph::Cas,
    original: crate::graph::LogicalId,
    record: &ScalarKernelRecord,
    input: &ScalarKernelInput,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(3, AT)?;
    input.center.check(budget, AT)?;
    input.radius.check(budget, AT)?;
    if input.radius.is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    let lower = input.center.sub(&input.radius, budget)?;
    let upper = input.center.add(&input.radius, budget)?;
    let covers_lower = record.lower.compare(&lower, budget)? != Ordering::Greater;
    let covers_upper = upper.compare(&record.upper, budget)? != Ordering::Greater;
    if !(covers_lower && covers_upper) {
        return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
    }
    match &input.value {
        ScalarKernelInputValue::Rational(value) => {
            value.check(budget, AT)?;
            relations::contains(record, value, budget)
        }
        ScalarKernelInputValue::PiMultiple {
            factor,
            certificate,
        } => {
            factor.check(budget, AT)?;
            super::cache::validate_pi(cas, original, certificate, budget)?;
            relations::contains(record, &factor.mul(&certificate.lower, budget)?, budget)?;
            relations::contains(record, &factor.mul(&certificate.upper, budget)?, budget)
        }
    }
}

/// Reconstruct the closed coherent source, never the numerical center.
pub(super) fn exact(
    input: &ScalarKernelInput,
    budget: &mut Budget,
) -> Result<ExactConstant, Fault> {
    match &input.value {
        ScalarKernelInputValue::Rational(value) => Ok(value.clone().into()),
        ScalarKernelInputValue::PiMultiple { factor, .. } => {
            ExactConstant::named(Constant::Pi, budget)?.mul(&factor.clone().into(), budget)
        }
    }
}

/// Conservative complete input ownership before any replay or copying.
pub(super) fn bytes(input: &ScalarKernelInput) -> u64 {
    let source = match &input.value {
        ScalarKernelInputValue::Rational(value) => number_bytes(value),
        ScalarKernelInputValue::PiMultiple {
            factor,
            certificate,
        } => [
            &certificate.lower,
            &certificate.upper,
            &certificate.sums[0],
            &certificate.sums[1],
        ]
        .into_iter()
        .fold(
            128_u64.saturating_add(number_bytes(factor)),
            |total, value| total.saturating_add(number_bytes(value)),
        ),
    };
    128_u64
        .saturating_add(number_bytes(&input.center))
        .saturating_add(number_bytes(&input.radius))
        .saturating_add(source)
}
