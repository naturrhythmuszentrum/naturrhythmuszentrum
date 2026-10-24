// SPDX-License-Identifier: AGPL-3.0-only
//! One mapped-selection owner per process, using the same fixture as public acceptance tests.

#[path = "../src/testing/mapped.rs"]
#[allow(
    dead_code,
    reason = "the measurement binary uses the successful shared fixture subset"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared acceptance support remains private to this example"
)]
mod mapped_testing;
#[path = "../src/testing/readings.rs"]
#[allow(
    dead_code,
    reason = "only declarations needed by the selected fixture are used"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact builders remain private to this example"
)]
mod reading_testing;

use std::io::{self, Write};

/// Setup is separate from the selected operation clock; process RSS includes both.
fn main() -> reading_testing::TestResult {
    let selector = std::env::args()
        .nth(1)
        .ok_or("expected radius or covariance")?;
    let covariance = match selector.as_str() {
        "radius" => false,
        "covariance" => true,
        _ => return Err("expected radius or covariance".into()),
    };
    let case = mapped_testing::case::<f64>(covariance)?;
    let execution = case.execution(&mut reading_testing::ledger()?)?;
    let selection = nrz_matrix::ReadingSelection::new(
        case.bindings[1].clone(),
        vec![execution],
        nrz_scalar::LiteralLimits::default(),
        &mut reading_testing::ledger()?,
    )?;
    let context = nrz_scalar::ArithmeticContext::new()?;
    let mut budget = reading_testing::ledger()?;
    let started = std::time::Instant::now();
    let evaluated = case
        .heap
        .select_mapped_reading(&selection, &context, &mut budget)?;
    let seconds = started.elapsed().as_secs_f64();
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "owner={selector} arch={} os={} precision=binary64 profile=dev(opt-level=2) work={} owned_bytes={} retained_allowance={} seconds={seconds:.9}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        evaluated.work(),
        evaluated.owned_bytes(),
        selection.retained_payload_bytes()
    )?;
    output.flush()?;
    Ok(())
}
