// SPDX-License-Identifier: AGPL-3.0-only
//! The public measurement executable and its controls share exactly the same implementation.
#[path = "../examples/prepared_exponential_measurements.rs"]
#[allow(
    dead_code,
    reason = "the integration target calls controls rather than the CLI"
)]
mod harness;
