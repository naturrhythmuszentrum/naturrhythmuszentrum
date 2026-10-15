// SPDX-License-Identifier: AGPL-3.0-only
//! Public conformance laws and heap lifecycles use one test target.

#[path = "../src/testing/public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public fixture helpers are shared only within this test target"
)]
mod testing;

#[path = "../src/testing/shapes.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shape fixtures are shared only within this public test target"
)]
mod shape_testing;

mod conformance;
mod integration;

use nrz_matrix as test_api;

#[path = "../src/testing/binding.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixtures are shared across public test modules"
)]
mod binding_testing;
