// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone prepared exponential phases; platform functions are uncertified baselines.

#[path = "prepared_exponential/support.rs"]
#[allow(
    dead_code,
    reason = "shared exact fixture builders include unused point helpers"
)]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "fixture helpers remain private to this executable"
    )
)]
mod support;
#[path = "prepared_exponential/trace.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "observer support is private to the harness"
)]
mod trace;

#[path = "prepared_exponential/common.rs"]
#[allow(
    clippy::redundant_pub_crate,
    reason = "shared at different module depths in example and integration target"
)]
mod common;
#[cfg(test)]
use common::admitted_inputs;
use common::{Snapshot, inputs, snapshot_exp, snapshot_expm1};
#[cfg(test)]
#[path = "prepared_exponential/sampling.rs"]
#[allow(dead_code, reason = "controls exercise adapters rather than their CLI")]
#[allow(
    clippy::redundant_pub_crate,
    reason = "shared at different module depths in example and integration target"
)]
mod sampling;

use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Outcome, Policy};
use nrz_matrix::{PreparedExp, PreparedExpm1};
use nrz_scalar::{ArithmeticContext, ExpFailure, ExpInput, Expm1Failure, Expm1Input};
use std::hint::black_box;
use std::io::{self, Write};
use std::time::Instant;
use support::{Fixture, TestResult, broad, ledger};

/// All tests in this executable serialize dispatcher interest changes.
#[cfg(test)]
static DISPATCHERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A completed operation's independent clocks and existing logical accounting.
#[derive(Debug, PartialEq)]
struct Observation {
    /// Ordered complete numerical results, separate from the timing checksum.
    results: Vec<Snapshot>,
    /// Fixed completion boundary, not a dynamic input identity.
    phase: &'static str,
    /// End minus start of the caller's exact ledger.
    work: Option<u64>,
    /// Logical retained payload or materialized inventory, not physical peak RSS.
    bytes: Option<u64>,
    /// Wall time including enabled observer callbacks.
    seconds: f64,
    /// Every output is consumed; this reproducible checksum is not a reference value.
    checksum: u64,
}

/// Compare the actual timed native results with ordered materialized batch results.
fn compare_batch(rows: &[Observation], phase: &str, batch: &[Snapshot]) -> TestResult {
    let native = rows
        .iter()
        .find(|row| row.phase == phase)
        .ok_or("native snapshot missing")?;
    if native.results != batch {
        return Err("ordered complete batch readings".into());
    }
    Ok(())
}

/// One process emits one finite population; repeat processes for uncertainty.
fn main() -> TestResult {
    let mut args = std::env::args().skip(1);
    let count: u32 = args.next().ok_or("expected cell count")?.parse()?;
    let enabled = match args.next().as_deref() {
        Some("enabled") => true,
        Some("disabled") => false,
        _ => return Err("expected enabled or disabled".into()),
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    write_run(count, enabled, &mut out)
}

/// Run the complete public path through its printed completion rows.
fn write_run(count: u32, enabled: bool, out: &mut impl Write) -> TestResult {
    let (rows, receipt) = run(count, enabled)?;
    writeln!(
        out,
        "owner=prepared-exponentials cells={count} tracing={enabled} \
         arch={} os={} profile={} environment={}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        std::env::var("NRZ_MEASUREMENT_ENV").unwrap_or_else(|_| "unspecified".into())
    )?;
    for row in rows {
        writeln!(
            out,
            "phase={} work={:?} logical_bytes={:?} seconds={:.9} checksum={}",
            row.phase, row.work, row.bytes, row.seconds, row.checksum
        )?;
    }
    if let Some(receipt) = receipt {
        writeln!(
            out,
            "observer_capacity={} observer={:?}",
            receipt.observer.capacity, receipt.observer
        )?;
        writeln!(
            out,
            "complete={} ledgers={:?} transfers={:?}",
            receipt.complete, receipt.ledgers, receipt.transfers
        )?;
        writeln!(out, "boundary_work={:?}", receipt.boundary_work)?;
        for (key, value) in receipt.receipts {
            writeln!(out, "work_receipt={key:?} value={value:?}")?;
        }
        for (key, value) in receipt.clocks {
            writeln!(
                out,
                "clock={key:?} seconds={:.9} calls={}",
                value.0.as_secs_f64(),
                value.1
            )?;
        }
        writeln!(out, "counters={:?}", receipt.counters)?;
    }
    out.flush()?;
    Ok(())
}

/// Observation loss invalidates the measurement, never changes the numerical outcome.
fn run(count: u32, enabled: bool) -> TestResult<(Vec<Observation>, Option<trace::Report>)> {
    let values = inputs(count)?;
    if enabled {
        let (rows, receipt) = trace::observed(4096, || measure(&values));
        for ((boundary, _), work) in &receipt.boundary_work {
            if boundary.starts_with("native-") && *work != 0 {
                return Err("native boundary consumed exact Work".into());
            }
        }
        if !receipt.complete {
            return Err("incomplete tracing receipt".into());
        }
        Ok((rows?, Some(receipt)))
    } else {
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            Ok((measure(&values)?, None))
        })
    }
}

/// Start immediately before the operation; stop after its consumed result, before rendering.
fn timed<T>(phase: &'static str, run: impl FnOnce() -> TestResult<T>) -> TestResult<(T, f64)> {
    let started = Instant::now();
    let result = {
        let _span = tracing::trace_span!("nrz.measure", family = phase, revision = 1_u64).entered();
        run()
    }?;
    Ok((result, started.elapsed().as_secs_f64()))
}

/// Counter events report the exact engine's own counts, not a timing-derived surrogate.
fn counters(counts: &Counts, phase: &str) {
    for (operation, count) in [
        ("CAS preparations", counts.preparations),
        ("CAS proposals", counts.proposals),
        ("CAS checks", counts.checks),
        ("CAS lookups", counts.lookups),
        ("CAS publications", counts.publications),
    ] {
        tracing::event!(
            name: "nrz.measure.counter",
            target: "nrz::work",
            tracing::Level::TRACE,
            family = phase,
            operation,
            count
        );
    }
}

/// Fixture and original-claim construction precede preparation; each row names its own boundary.
#[expect(
    clippy::too_many_lines,
    reason = "adjacent clocks expose the complete lifetime of one frozen authority \
              and its borrowed outputs"
)]
fn measure(values: &[f64]) -> TestResult<Vec<Observation>> {
    let context = ArithmeticContext::new()?;
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let mut budget = ledger()?;
    let mut rows = Vec::new();
    for phase in ["prepare-cold", "prepare-warm", "prepare-changed-target"] {
        let mut request = request.clone();
        if phase == "prepare-changed-target" {
            request.bits = 39;
        }
        let mut counts = Counts::default();
        let before = budget.usage().work;
        let (outcome, seconds) = timed(phase, || {
            Ok(fixture
                .cas
                .prepare_expm1(&request, Policy::default(), &mut budget, &mut counts)?)
        })?;
        let Outcome::Ready(prepared) = outcome else {
            return Err("fixture basis unavailable".into());
        };
        counters(&counts, phase);
        let hit = phase == "prepare-warm";
        if hit && (counts.proposals != 0 || counts.checks != 0) {
            return Err("warm authority rebuilt".into());
        }
        if !hit && (counts.proposals == 0 || counts.checks == 0) {
            return Err("cold authority not constructed".into());
        }
        tracing::event!(
            name: "nrz.measure.counter",
            target: "nrz::work",
            tracing::Level::TRACE,
            family = phase,
            operation = if hit { "cache hit" } else { "cache miss" },
            count = 1_u64
        );
        rows.push(Observation {
            results: Vec::new(),
            phase,
            seconds,
            work: Some(budget.usage().work - before),
            bytes: Some(fixture.cas.retained_judgment_bytes()),
            checksum: u64::from(prepared.proof().degree),
        });
    }
    let before = budget.usage().work;
    let (plan, seconds) = timed("admit-expm1", || {
        Ok(PreparedExpm1::new(
            &mut fixture.cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut budget,
            &mut Counts::default(),
        )?)
    })?;
    rows.push(Observation {
        results: Vec::new(),
        phase: "admit-expm1",
        seconds,
        work: Some(budget.usage().work - before),
        bytes: Some(plan.owned_bytes()),
        checksum: u64::from(plan.authority().proof().degree),
    });
    let mut replay = Fixture::new()?;
    let replay_request = broad(&mut replay, 40)?;
    let mut replay_budget = ledger()?;
    let (checked, seconds) = timed("independent-replay", || {
        Ok(replay.cas.check_expm1(
            &replay_request,
            Policy::default(),
            plan.authority().proof(),
            &mut replay_budget,
            &mut Counts::default(),
        )?)
    })?;
    if checked.proof() != plan.authority().proof() {
        return Err("replay changed evidence".into());
    }
    rows.push(Observation {
        results: Vec::new(),
        phase: "independent-replay",
        seconds,
        work: Some(replay_budget.usage().work),
        bytes: Some(replay.cas.retained_judgment_bytes()),
        checksum: u64::from(checked.proof().degree),
    });
    let (operands, seconds) = timed("import-binary64", || {
        Ok(values
            .iter()
            .map(|x| Expm1Input {
                endpoints: [*x, *x],
                center: *x,
                bits: 40,
            })
            .collect::<Vec<_>>())
    })?;
    rows.push(Observation {
        results: Vec::new(),
        phase: "import-binary64",
        seconds,
        work: None,
        bytes: Some(std::mem::size_of_val(operands.as_slice()) as u64),
        checksum: operands.len() as u64,
    });
    let native_work = budget.usage().work;
    let mut native_results = Vec::with_capacity(operands.len());
    let (checksum, seconds) = timed("native-expm1-reuse", || {
        let mut sum = 0;
        for input in &operands {
            let reading = black_box(plan.evaluate(black_box(*input), &context)?.reading());
            sum ^= reading.center.to_bits();
            native_results.push(snapshot_expm1(Ok(reading)));
        }
        Ok(sum)
    })?;
    rows.push(Observation {
        results: native_results,
        phase: "native-expm1-reuse",
        seconds,
        work: Some(budget.usage().work - native_work),
        bytes: Some(plan.owned_bytes()),
        checksum,
    });
    let cases = [
        Expm1Input {
            endpoints: [-2.0, 0.0],
            center: -1.0,
            bits: 40,
        },
        Expm1Input {
            endpoints: [0.5, 0.5],
            center: 0.5,
            bits: 40,
        },
        Expm1Input {
            endpoints: [-1.0, -1.0],
            center: -1.0,
            bits: 39,
        },
        Expm1Input {
            endpoints: [-f64::from_bits(1), -f64::from_bits(1)],
            center: -f64::from_bits(1),
            bits: 40,
        },
    ];
    let (case_results, seconds) = timed("expm1-box-and-refusals", || {
        let results = cases.map(|input| {
            plan.evaluate(input, &context)
                .map(|output| black_box(output.reading()))
        });
        let expected = [
            Expm1Failure::OutsideDomain,
            Expm1Failure::Target,
            Expm1Failure::BelowNormalOperand,
        ];
        if results[0].is_err()
            || results[1..]
                .iter()
                .zip(expected)
                .any(|(result, expected)| result.as_ref().err() != Some(&expected))
        {
            return Err("box/refusal association".into());
        }
        Ok(results)
    })?;
    rows.push(Observation {
        results: case_results.into_iter().map(snapshot_expm1).collect(),
        phase: "expm1-box-and-refusals",
        seconds,
        work: None,
        bytes: Some(0),
        checksum: case_results.iter().filter(|r| r.is_err()).count() as u64,
    });
    let before = budget.usage().work;
    let (outputs, seconds) = timed("batch-expm1", || {
        Ok(plan.evaluate_batch(&operands, &context, &mut budget)?)
    })?;
    let mut batch_results = Vec::with_capacity(outputs.len());
    let (batch_checksum, reading_seconds) = timed("read-expm1-outputs", || {
        outputs
            .iter()
            .try_fold(0, |sum, output| -> TestResult<u64> {
                let reading = black_box(output.as_ref().map_err(ToString::to_string)?.reading());
                batch_results.push(snapshot_expm1(Ok(reading)));
                Ok(sum ^ reading.center.to_bits())
            })
    })?;
    rows.push(Observation {
        results: Vec::new(),
        phase: "read-expm1-outputs",
        seconds: reading_seconds,
        work: None,
        bytes: Some(std::mem::size_of_val(outputs.as_slice()) as u64),
        checksum: batch_checksum,
    });
    if checksum != batch_checksum {
        return Err("batch result association".into());
    }
    if outputs.len() != operands.len() {
        return Err("batch cardinality".into());
    }
    compare_batch(&rows, "native-expm1-reuse", &batch_results)?;
    rows.push(Observation {
        results: batch_results,
        phase: "batch-expm1",
        seconds,
        work: Some(budget.usage().work - before),
        bytes: Some(std::mem::size_of_val(outputs.as_slice()) as u64),
        checksum: batch_checksum,
    });
    let started = Instant::now();
    {
        let _span = tracing::trace_span!("nrz.measure", family = "release-expm1", revision = 1_u64)
            .entered();
        drop(outputs);
        drop(plan);
    }
    let seconds = started.elapsed().as_secs_f64();
    rows.push(Observation {
        results: Vec::new(),
        phase: "release-expm1",
        seconds,
        work: None,
        bytes: Some(0),
        checksum: 0,
    });
    let before = budget.usage().work;
    let (plan, seconds) = timed("admit-exp", || {
        // The auxiliary policy charges before the basis emits its first snapshot.
        budget.with_work_phase("measurement", "admit-exp", |budget| {
            PreparedExp::new(
                &mut fixture.cas,
                &request,
                [-2.0, 0.0],
                Policy::default(),
                &context,
                budget,
                &mut Counts::default(),
            )
            .map_err(|error| error.to_string().into())
        })
    })?;
    rows.push(Observation {
        results: Vec::new(),
        phase: "admit-exp",
        seconds,
        work: Some(budget.usage().work - before),
        bytes: None,
        checksum: u64::from(plan.basis().proof().degree),
    });
    let operands: Vec<_> = operands
        .iter()
        .map(|input| ExpInput {
            endpoints: input.endpoints,
            center: input.center,
            bits: input.bits,
        })
        .collect();
    let native_work = budget.usage().work;
    let mut native_results = Vec::with_capacity(operands.len());
    let (checksum, seconds) = timed("native-exp-reuse", || {
        let mut sum = 0;
        for input in &operands {
            let reading = black_box(plan.evaluate(black_box(*input), &context)?.reading());
            sum ^= reading.center.to_bits();
            native_results.push(snapshot_exp(Ok(reading)));
        }
        Ok(sum)
    })?;
    rows.push(Observation {
        results: native_results,
        phase: "native-exp-reuse",
        seconds,
        work: Some(budget.usage().work - native_work),
        bytes: None,
        checksum,
    });
    let cases = [
        ExpInput {
            endpoints: [-2.0, 0.0],
            center: -1.0,
            bits: 40,
        },
        ExpInput {
            endpoints: [0.5, 0.5],
            center: 0.5,
            bits: 40,
        },
        ExpInput {
            endpoints: [-1.0, -1.0],
            center: -1.0,
            bits: 39,
        },
        ExpInput {
            endpoints: [-f64::from_bits(1), -f64::from_bits(1)],
            center: -f64::from_bits(1),
            bits: 40,
        },
    ];
    let (case_results, seconds) = timed("exp-box-and-refusals", || {
        let results = cases.map(|input| {
            plan.evaluate(input, &context)
                .map(|output| black_box(output.reading()))
        });
        let expected = [
            ExpFailure::Basis(Expm1Failure::OutsideDomain),
            ExpFailure::Target,
            ExpFailure::Basis(Expm1Failure::BelowNormalOperand),
        ];
        if results[0].is_err()
            || results[1..]
                .iter()
                .zip(expected)
                .any(|(result, expected)| result.as_ref().err() != Some(&expected))
        {
            return Err("box/refusal association".into());
        }
        Ok(results)
    })?;
    rows.push(Observation {
        results: case_results.into_iter().map(snapshot_exp).collect(),
        phase: "exp-box-and-refusals",
        seconds,
        work: None,
        bytes: Some(0),
        checksum: case_results.iter().filter(|r| r.is_err()).count() as u64,
    });
    let before = budget.usage().work;
    let (outputs, seconds) = timed("batch-exp", || {
        Ok(plan.evaluate_batch(&operands, &context, &mut budget)?)
    })?;
    let mut batch_results = Vec::with_capacity(outputs.len());
    let (batch_checksum, reading_seconds) = timed("read-exp-outputs", || {
        outputs
            .iter()
            .try_fold(0, |sum, output| -> TestResult<u64> {
                let reading = black_box(output.as_ref().map_err(ToString::to_string)?.reading());
                batch_results.push(snapshot_exp(Ok(reading)));
                Ok(sum ^ reading.center.to_bits())
            })
    })?;
    rows.push(Observation {
        results: Vec::new(),
        phase: "read-exp-outputs",
        seconds: reading_seconds,
        work: None,
        bytes: Some(std::mem::size_of_val(outputs.as_slice()) as u64),
        checksum: batch_checksum,
    });
    if checksum != batch_checksum {
        return Err("exp batch result association".into());
    }
    if outputs.len() != operands.len() {
        return Err("batch cardinality".into());
    }
    compare_batch(&rows, "native-exp-reuse", &batch_results)?;
    rows.push(Observation {
        results: batch_results,
        phase: "batch-exp",
        seconds,
        work: Some(budget.usage().work - before),
        bytes: Some(std::mem::size_of_val(outputs.as_slice()) as u64),
        checksum: batch_checksum,
    });
    let started = Instant::now();
    {
        let _span =
            tracing::trace_span!("nrz.measure", family = "release-exp", revision = 1_u64).entered();
        drop(outputs);
        drop(plan);
    }
    let seconds = started.elapsed().as_secs_f64();
    rows.push(Observation {
        results: Vec::new(),
        phase: "release-exp",
        seconds,
        work: None,
        bytes: Some(0),
        checksum: 0,
    });
    for phase in ["uncertified-exp", "uncertified-expm1"] {
        let (checksum, seconds) = timed(phase, || {
            Ok(values.iter().fold(0, |sum, x| {
                sum ^ black_box(if phase == "uncertified-exp" {
                    black_box(*x).exp()
                } else {
                    black_box(*x).exp_m1()
                })
                .to_bits()
            }))
        })?;
        rows.push(Observation {
            results: Vec::new(),
            phase,
            seconds,
            work: None,
            bytes: None,
            checksum,
        });
    }
    Ok(rows)
}

#[cfg(test)]
#[path = "../tests/prepared_exponential_measurements/controls.rs"]
mod tests;
