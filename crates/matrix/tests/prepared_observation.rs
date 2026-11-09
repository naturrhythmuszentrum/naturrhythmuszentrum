// SPDX-License-Identifier: AGPL-3.0-only
//! Isolated subscriber controls: every test holds the shared dispatcher guard.
#[path = "../examples/prepared_exponential/support.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture helpers remain private to this integration target"
)]
mod support;

use nrz_bindings::readings::algebra::FaultKind;
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use nrz_bindings::readings::algebra::limits::{Budget, BudgetKind, Limits};
use nrz_matrix::PreparedExpm1;
use nrz_scalar::{ArithmeticContext, Expm1Input};
use support::{Fixture, TestResult, broad, ledger};

#[path = "prepared_observation/observation.rs"]
mod observation;

/// A represented point with the fixture's relative target.
const fn point(x: f64) -> Expm1Input {
    Expm1Input {
        endpoints: [x, x],
        center: x,
        bits: 40,
    }
}
