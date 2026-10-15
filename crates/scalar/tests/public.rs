// SPDX-License-Identifier: AGPL-3.0-only
//! Independent public conformance laws and numerical lifecycles.

mod conformance;
mod integration;
#[path = "../src/testing/public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "oracle helpers are shared only within this public test target"
)]
mod testing;
