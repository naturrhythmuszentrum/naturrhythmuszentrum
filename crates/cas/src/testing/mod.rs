// SPDX-License-Identifier: AGPL-3.0-only
//! Shared setup for crate tests with internal access.
//!
//! Compiled only in the library test binary.
//!
//! Inline unit tests keep their setup local and never import this harness.

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
pub(crate) mod common;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
pub(crate) mod frontier;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
pub(crate) mod frozen;
#[expect(clippy::redundant_pub_crate, reason = "shared crate case fixtures")]
pub(crate) mod native;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared crate boundary evidence fixtures"
)]
pub(crate) mod boundary_native;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared internal distribution evidence"
)]
pub(crate) mod distribution_native;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared admission fixtures stay crate-visible"
)]
pub(crate) mod admission;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_numeric;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_graph;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_coefficients;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_elementary;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_orbits;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_blind;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_rational;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_linear;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_local;

#[expect(
    clippy::redundant_pub_crate,
    reason = "family crate tests share original public sort fixtures"
)]
#[expect(
    dead_code,
    reason = "the same fixture also serves public admission tests"
)]
pub(crate) mod sorts;

#[expect(
    clippy::redundant_pub_crate,
    reason = "replay tests share ledgers and syntax-only proposals"
)]
pub(crate) mod replay_answer;

#[expect(
    clippy::redundant_pub_crate,
    reason = "crate tests attribute replay Work without changing production accounting"
)]
pub(crate) mod work_trace;
