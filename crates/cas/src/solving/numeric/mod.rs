// SPDX-License-Identifier: AGPL-3.0-only
//! Preflighted exact Gaussian arithmetic and verified integer-root candidates.
//!
//! Arithmetic scratch values are not graph objects or theorem certificates.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the next coefficient lane consumes this numeric leaf"
    )
)]

mod gaussian;
#[expect(
    clippy::redundant_pub_crate,
    reason = "numeric bounds are shared within the crate"
)]
mod plan;
#[expect(
    clippy::redundant_pub_crate,
    reason = "root helpers are shared within the crate"
)]
mod powers;

use num_complex::Complex;

use crate::budget::Budget;
use crate::error::Fault;
use crate::number::Rational;

/// A Gaussian rational with no infallible arithmetic, equality or copying traits.
#[derive(Debug)]
pub(crate) struct Gaussian {
    /// Only componentwise prepaid operations may inspect or copy this pair.
    value: Complex<Rational>,
}

/// This leaf supplies arithmetic; it does not enable any theorem schema.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Return the exact signed integer root, or `None` for a nonpower or negative even radicand.
/// Order zero is invalid; all candidate arithmetic and verification are prepaid.
pub(crate) fn integer_root(
    value: &num_bigint::BigInt,
    order: u32,
    budget: &mut Budget,
) -> Result<Option<num_bigint::BigInt>, Fault> {
    powers::integer_root(value, order, budget)
}

/// A reduced rational is a perfect power precisely when both magnitudes are.
/// Returns the real odd root for negative values; negative even roots return `None`.
pub(crate) fn rational_root(
    value: &Rational,
    order: u32,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    powers::rational_root(value, order, budget)
}
