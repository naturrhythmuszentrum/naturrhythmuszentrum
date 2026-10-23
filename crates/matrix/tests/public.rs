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

#[path = "../src/testing/conversion_oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent numerical oracle is shared by conversion tests"
)]
mod conversion_oracle;
#[path = "../src/testing/conversion.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "conversion fixtures are shared across public test modules"
)]
mod conversion_testing;

#[path = "../src/testing/kernel_oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent rational truth is shared across public kernel tests"
)]
mod kernel_oracle;
#[path = "../src/testing/kernels.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "kernel fixtures are shared across public test modules"
)]
mod kernel_testing;

#[path = "../src/testing/solve.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "solve fixtures are shared across the corresponding test layers"
)]
mod solve_testing;

#[path = "../src/testing/solve_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact elimination and independent backend checks are shared by public solve tests"
)]
mod solve_public;
#[path = "../src/testing/statistics.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "statistical fixtures and rational oracles are shared across test modules"
)]
mod statistics_testing;

#[path = "../src/testing/statistics_oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent statistical integer comparisons are shared across public tests"
)]
mod statistics_oracle;

#[path = "../src/testing/solve_statistics.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "solve source fixtures are shared across test modules"
)]
mod solve_statistics_testing;

#[path = "../src/testing/recovery.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "bounded solve policy fixtures are shared across test modules"
)]
mod recovery_testing;

#[path = "../src/testing/readings.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact reading fixtures are shared across the corresponding test layers"
)]
mod reading_testing;

#[path = "../src/testing/copies.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact reading fixtures are shared across the corresponding test layers"
)]
mod copy_testing;
