// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact Machin certificates, without numerical-owner authority.

use std::cmp::Ordering;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::number::Rational;
use crate::proof::{CONSTANT_SCHEMAS, LemmaSchema};

/// Every certificate operation spends the caller's cumulative ledger here.
const AT: Location = Location::new("Machin pi bounds");

/// An untrusted finite expansion of `16 atan(1/5) - 4 atan(1/239)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PiBoundCertificate {
    /// Semantic theorem revision; only revision one is supported.
    pub version: u32,
    /// Number of terms for divisors five and 239, respectively; zero is permitted.
    pub terms: [u32; 2],
    /// Claimed exact alternating partial sums, checked independently term by term.
    pub sums: [Rational; 2],
    /// Claimed included lower bound for mathematical pi.
    pub lower: Rational,
    /// Claimed included upper bound for mathematical pi.
    pub upper: Rational,
}

/// Sealed evidence of the constant theorem, retaining its full checked instance.
#[derive(Clone, Debug)]
pub struct CheckedPiBounds {
    /// Immutable expansion, exact sums, endpoints and theorem revision.
    contract: PiBoundCertificate,
    /// Prepaid logical ownership of this returned certificate.
    bytes: u64,
}

impl CheckedPiBounds {
    /// Borrow the independently checked finite expansion and its outward bounds.
    #[must_use]
    pub const fn contract(&self) -> &PiBoundCertificate {
        &self.contract
    }

    /// The named theorem is a dependency of this checked instance, never its authority.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static LemmaSchema {
        &CONSTANT_SCHEMAS[0]
    }

    /// Logical bytes retained by the returned evidence.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl PiBoundCertificate {
    /// Reconstruct both exact sums and their signed next-term remainder bounds.
    ///
    /// No scalar certificate, floating approximation or caller success flag is used.
    /// The returned instance cites the compiled Machin theorem and its revision.
    /// # Errors
    /// Refuses unknown revisions, false partial sums, inward or reversed bounds,
    /// unsupported arithmetic sizes and exhausted cumulative resources.
    pub fn check(&self, budget: &mut Budget) -> Result<CheckedPiBounds, Fault> {
        let bytes = self.validate(budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
        budget.work(bytes, AT)?;
        Ok(CheckedPiBounds {
            contract: self.clone(),
            bytes,
        })
    }

    /// Validate retained proof data without constructing a redundant owned receipt.
    pub(crate) fn validate(&self, budget: &mut Budget) -> Result<u64, Fault> {
        budget.work(8, AT)?;
        if self.version != CONSTANT_SCHEMAS[0].version {
            return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
        }
        let mut bytes = 128_u64;
        for value in [&self.lower, &self.upper, &self.sums[0], &self.sums[1]] {
            value.check(budget, AT)?;
            bytes = bytes.saturating_add(number_bytes(value));
        }
        let scratch = budget
            .limits()
            .component_bits
            .div_ceil(8)
            .saturating_mul(64)
            .saturating_add(2_048);
        budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
            self.reduce(budget)
        })?;
        Ok(bytes)
    }

    /// The fixed positive coefficients reverse only the subtracted arctangent interval.
    fn reduce(&self, budget: &mut Budget) -> Result<(), Fault> {
        if self.lower.compare(&self.upper, budget)? == Ordering::Greater {
            return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
        }
        let first = series(5, self.terms[0], &self.sums[0], budget)?;
        let second = series(239, self.terms[1], &self.sums[1], budget)?;
        let sixteen = Rational::from_ratio(16, 1, budget)?;
        let four = Rational::from_ratio(4, 1, budget)?;
        let lower = first
            .0
            .mul(&sixteen, budget)?
            .sub(&second.1.mul(&four, budget)?, budget)?;
        let upper = first
            .1
            .mul(&sixteen, budget)?
            .sub(&second.0.mul(&four, budget)?, budget)?;
        let outward_lower = self.lower.compare(&lower, budget)? != Ordering::Greater;
        let outward_upper = upper.compare(&self.upper, budget)? != Ordering::Greater;
        if outward_lower && outward_upper {
            Ok(())
        } else {
            Err(Fault::invalid(InvalidCode::DecisionBox, AT))
        }
    }
}

/// Recompute a finite alternating sum; its omitted term gives a one-sided remainder.
fn series(
    divisor: u64,
    terms: u32,
    expected: &Rational,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    let x = Rational::from_ratio(1, divisor, budget)?;
    let square = x.mul(&x, budget)?;
    let mut power = x;
    let mut sum = Rational::zero();
    for index in 0..terms {
        budget.work(1, AT)?;
        let denominator = Rational::from_ratio(2 * i64::from(index) + 1, 1, budget)?;
        let term = power.div(&denominator, budget)?;
        sum = if index.is_multiple_of(2) {
            sum.add(&term, budget)?
        } else {
            sum.sub(&term, budget)?
        };
        power = power.mul(&square, budget)?;
    }
    if sum.compare(expected, budget)? != Ordering::Equal {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    let denominator = Rational::from_ratio(2 * i64::from(terms) + 1, 1, budget)?;
    let tail = power.div(&denominator, budget)?;
    if terms.is_multiple_of(2) {
        let upper = sum.add(&tail, budget)?;
        Ok((sum, upper))
    } else {
        let lower = sum.sub(&tail, budget)?;
        Ok((lower, sum))
    }
}

/// Conservative allocation bound without copying the rational's private components.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}
