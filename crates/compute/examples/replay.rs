// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit bounded raw-file reanalysis; this example never collects measurements.

use std::{env, fs::File};

/// Reconstruct recorded analysis using the report's own settings and explicit read-byte cap.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let path = arguments
        .next()
        .ok_or("supply an external raw report path")?;
    let bytes = arguments.next().ok_or("supply the raw read-byte cap")?;
    let bytes: u64 = bytes.to_str().ok_or("byte cap must be UTF-8")?.parse()?;
    if arguments.next().is_some() {
        return Err("unexpected replay argument".into());
    }
    let report = nrz_compute::read_report(File::open(path)?, bytes)?;
    nrz_compute::replay::verify_raw_analysis(&report, |_| false)?;
    Ok(())
}
