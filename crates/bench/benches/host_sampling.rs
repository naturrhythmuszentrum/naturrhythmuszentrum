// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared native exponential and trivial-call observations in both sampling hosts.

mod support;

/// Keep fixture, sampling and output failures fallible at the executable boundary.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::run()
}
