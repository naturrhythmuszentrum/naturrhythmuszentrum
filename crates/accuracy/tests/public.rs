// SPDX-License-Identifier: AGPL-3.0-only
//! Public numerical conformance and bound-policy lifecycles.

mod conformance;
mod integration;
#[path = "../src/testing/public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent oracle helpers are shared only within this public test target"
)]
mod testing;
