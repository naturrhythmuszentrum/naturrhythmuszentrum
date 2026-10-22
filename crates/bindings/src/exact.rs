// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded exact descriptor arithmetic uses the engine's rational implementation.

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
