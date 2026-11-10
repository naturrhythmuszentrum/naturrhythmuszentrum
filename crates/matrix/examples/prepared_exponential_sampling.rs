// SPDX-License-Identifier: AGPL-3.0-only
//! Reproducible block sampling of six reusable prepared exponential phases.
#[path = "prepared_exponential/common.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private example fixtures"
)]
mod common;
#[path = "prepared_exponential/sampling.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private example adapters"
)]
mod sampling;
#[path = "prepared_exponential/sampling_json.rs"]
#[expect(clippy::redundant_pub_crate, reason = "private example exporter")]
mod sampling_json;
#[path = "prepared_exponential/support.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "private shared example fixtures"
)]
mod support;

use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use nrz_matrix::{PreparedExp, PreparedExpm1, sampling::sample};
use nrz_scalar::ArithmeticContext;
use sampling::{Adapter, Block, Phase, Prepared};
use std::io::{self, Write};
use support::{Fixture, TestResult, broad, ledger};

/// Prepare once and emit every phase's complete structured collection report.
fn main() -> TestResult {
    let mut args = std::env::args().skip(1);
    let count = args.next().ok_or("expected population size")?.parse()?;
    if args.next().is_some() {
        return Err("expected only population size; all settings are named and printed".into());
    }
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || run(count))
}

/// Each adapter borrows the same immutable plans and unsorted input block.
fn run(count: u32) -> TestResult {
    let block = Block::new(common::inputs(count)?);
    let arithmetic = ArithmeticContext::new()?;
    let mut left = Fixture::new()?;
    let request = broad(&mut left, 40)?;
    let expm1 = PreparedExpm1::new(
        &mut left.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let mut right = Fixture::new()?;
    let request = broad(&mut right, 40)?;
    let exp = PreparedExp::new(
        &mut right.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut ledger()?,
        &mut Counts::default(),
    )
    .map_err(|error| error.to_string())?;
    let prepared = Prepared {
        expm1: &expm1,
        exp: &exp,
        arithmetic: &arithmetic,
    };
    let shape = [block.values.len()];
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "{{\"preparation_sampled\":false,\"tracing\":\"disabled\",\"reports\":["
    )?;
    for (index, phase) in [
        Phase::NativeExpm1,
        Phase::NativeExp,
        Phase::BatchExpm1,
        Phase::BatchExp,
        Phase::PlatformExpm1,
        Phase::PlatformExp,
    ]
    .into_iter()
    .enumerate()
    {
        if index != 0 {
            writeln!(out, ",")?;
        }
        let mut adapter = Adapter {
            phase,
            allowances: sampling::allowances(),
            inspect: None,
        };
        let report = sample(
            &prepared,
            &sampling::representative(&block, phase, &shape),
            &mut adapter,
            sampling::settings()?,
        )?;
        sampling_json::write(&mut out, &report)?;
    }
    writeln!(out, "]}}")?;
    out.flush()?;
    Ok(())
}
