// SPDX-License-Identifier: AGPL-3.0-only
//! Small shared result, budget and scalar helpers.
//!
//! Compiled only in the public test target.

use crate::test_api::{AtomicSort, Budget, Fault, Field, Limits, Sort};

/// Setup failures retain their original error for the test runner.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A fresh ledger using every production default ceiling.
pub(crate) fn default_budget() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// The real scalar sort, without physical interpretation.
pub(crate) const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
