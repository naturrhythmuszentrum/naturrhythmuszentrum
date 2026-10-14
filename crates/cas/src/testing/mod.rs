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
