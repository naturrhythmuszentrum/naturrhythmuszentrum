// SPDX-License-Identifier: AGPL-3.0-only
//! Public-only fixtures; this harness never imports private units implementation state.

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public test fixtures stay crate-visible inside the private harness"
)]
#[path = "declarations.rs"]
pub(crate) mod declarations;
