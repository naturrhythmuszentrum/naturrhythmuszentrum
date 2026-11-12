// SPDX-License-Identifier: AGPL-3.0-only
//! Controls execute the same private harness as the rerunnable example.
#[path = "../examples/prepared_sigmoid_tanh_measurements.rs"]
#[allow(
    dead_code,
    reason = "the integration target exercises controls rather than the CLI"
)]
mod harness;
