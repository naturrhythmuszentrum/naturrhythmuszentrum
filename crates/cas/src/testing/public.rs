// SPDX-License-Identifier: AGPL-3.0-only
//! Shared public-API fixtures for conformance and integration tests.
//!
//! Compiled only in the public test target.
//!
//! Inline unit tests never import this harness. Crate-only fixture sources are not included.

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "budgets.rs"]
pub(crate) mod budgets;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "builders.rs"]
pub(crate) mod builders;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "common.rs"]
pub(crate) mod common;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "graph.rs"]
pub(crate) mod graph;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "pendulum.rs"]
pub(crate) mod pendulum;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "proofs.rs"]
pub(crate) mod proofs;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "representation.rs"]
pub(crate) mod representation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "requests.rs"]
pub(crate) mod requests;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "square_free.rs"]
pub(crate) mod square_free;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "wire.rs"]
pub(crate) mod wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "witness.rs"]
pub(crate) mod witness;
