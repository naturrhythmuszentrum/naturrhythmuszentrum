// SPDX-License-Identifier: AGPL-3.0-only
//! The existing cumulative phasor caller policy is shared without changing production defaults.

use super::reading_testing::TestResult;
use nrz_bindings::{Budget, Limits};

/// Explicit cumulative caller policy for correspondence, equation, references and publication.
pub(crate) fn policy() -> Limits {
    Limits {
        work: 16_000_000,
        ..Limits::default()
    }
}
/// One cumulative operation ledger; fixture declaration scaffolding retains its separate setup costs.
pub(crate) fn budget() -> TestResult<Budget> {
    Ok(Budget::new(policy())?)
}
