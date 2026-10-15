// SPDX-License-Identifier: AGPL-3.0-only
//! Exact fixture coefficients use the public CAS constructor.

use nrz_cas::{
    Fault, Rational,
    limits::{Budget, Limits},
};

/// Build a small exact coefficient with an independent default ledger.
pub(crate) fn rational(numerator: i64, denominator: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(numerator, denominator, &mut Budget::new(Limits::default())?)
}
