// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded exact descriptor arithmetic uses the engine's rational implementation.
//!
//! # Closed source-free constant grammar
//!
//! The closed constant grammar admits rational literals, pi, e, negation, arithmetic and integer powers. The CAS checks every original operand for finite real definedness before normalization, including divisors hidden by zero or cancellation; free symbols and unsupported operators refuse. Strict signs, exact rational subresults and same-sign sums establish positivity conservatively. A mixed expression such as `pi-1` remains unproved even though a stronger theorem could establish it. Existing CAS polynomial normalization and an independent coefficient equality check produce immutable exact graph values. Equality uses normalized structure, not names or a complete transcendental identity decision; rational literals retain their existing arithmetic ledgers.

use nrz_cas::{
    Fault, Rational,
    limits::{Budget, Limits},
};

/// A fresh ledger for an independent fixed-size descriptor operation.
pub(crate) fn budget() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Construct a small descriptor coefficient through the budgeted CAS constructor.
pub(crate) fn ratio(numerator: i64, denominator: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(numerator, denominator, &mut budget()?)
}
