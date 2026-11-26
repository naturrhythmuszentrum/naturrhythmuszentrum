// SPDX-License-Identifier: AGPL-3.0-only
//! Command-line entry point for the Naturrhythmuszentrum engine; commands are still to come.

#[cfg_attr(
    test,
    expect(
        clippy::missing_const_for_fn,
        reason = "under the test harness main is not the entry point, so Clippy asks for const; the command entry point stays an ordinary fn for I/O"
    )
)]
fn main() {}
