// SPDX-License-Identifier: AGPL-3.0-only
//! Public conformance laws and heap lifecycles use one test target.

#[path = "../src/testing/public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public fixture helpers are shared only within this test target"
)]
mod testing;

mod conformance;
mod integration;
