// SPDX-License-Identifier: AGPL-3.0-only
//! Print committed LC owner observations; documentation generation never executes this binary.

#[path = "../tests/integration/lc_acceptance/support/mod.rs"]
#[allow(
    dead_code,
    reason = "the example uses the measurement entry point of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the example shares private fixtures with public acceptance tests"
)]
mod support;

#[allow(
    dead_code,
    reason = "the example selects a subset of complete shared phasor fixture operations"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the example uses private shared fixture declarations"
)]
mod lc_acceptance;
#[path = "../src/testing/readings.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact reading fixtures are private to this measurement target"
)]
mod reading_testing;

use std::io::{self, Write};

/// Run one owner per process so server peak RSS belongs to that complete fixture and operation.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selector = std::env::args().nth(1).ok_or("expected an LC owner name")?;
    let observations = lc_acceptance::measure(&selector)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "owner={selector} arch={} os={} profile=dev(opt-level=2)",
        std::env::consts::ARCH,
        std::env::consts::OS
    )?;
    for observation in observations {
        writeln!(
            output,
            "phase={} work={} seconds={:.9} {}",
            observation.phase, observation.work, observation.seconds, observation.accounting
        )?;
    }
    output.flush()?;
    Ok(())
}
