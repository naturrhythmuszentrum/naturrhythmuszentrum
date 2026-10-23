// SPDX-License-Identifier: AGPL-3.0-only
//! Public-only fixtures; this harness never imports private bindings implementation state.

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public test fixtures stay crate-visible inside the private harness"
)]
#[path = "declarations.rs"]
pub(crate) mod declarations;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public pairing fixtures stay crate-visible inside the private harness"
)]
#[path = "pairing.rs"]
pub(crate) mod pairing;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public sensitivity fixtures stay crate-visible inside the private harness"
)]
#[path = "sensitivity.rs"]
pub(crate) mod sensitivity;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact definition fixtures stay crate-visible inside the private harness"
)]
#[path = "constants.rs"]
pub(crate) mod constants;

#[expect(
    clippy::redundant_pub_crate,
    reason = "physical reading fixtures are shared by public integration and conformance tests"
)]
#[path = "readings.rs"]
pub(crate) mod readings;

#[expect(
    clippy::redundant_pub_crate,
    reason = "coordinate normalization fixtures are shared by public contract and lifecycle tests"
)]
#[path = "normalization.rs"]
pub(crate) mod normalization;

#[expect(
    clippy::redundant_pub_crate,
    reason = "fixtures are shared across public test modules"
)]
pub(crate) mod lc_coordinates;
