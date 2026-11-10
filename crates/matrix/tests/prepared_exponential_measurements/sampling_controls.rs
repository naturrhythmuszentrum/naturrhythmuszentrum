// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic sampling of the exact shared block adapters.
use super::*;
#[path = "sampling_lifecycle.rs"]
mod lifecycle;
use nrz_matrix::sampling::{ReceiptStatus, SamplingClock, SamplingStop, sample_with_clock};
use sampling::{Adapter, Block, Phase, Prepared, Work};
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

thread_local! {
    /// Test-only disposal sink; every full result is compared outside the timer.
    static EXPECTED: RefCell<(Vec<Snapshot>, Vec<u64>, usize)> = const {
        RefCell::new((Vec::new(), Vec::new(), 0))
    };
}
/// A ticking clock makes every invocation positive without machine-speed assumptions.
#[derive(Debug, Default)]
struct Clock(Cell<u64>);
impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        let value = self.0.get();
        self.0.set(value + 1);
        Duration::from_micros(value)
    }
}
/// Compare all endpoints, errors and radii rather than a checksum.
/// # Panics
/// Panics if a sampled reading differs from the ordinary harness.
fn inspect(snapshots: &[Snapshot], baseline: &[u64]) {
    EXPECTED.with_borrow_mut(|(expected, platform, count)| {
        assert_eq!(snapshots, expected);
        assert_eq!(baseline, platform);
        *count += 1;
    });
}

/// Measure ordinary evaluation with the same empty registration inventory.
fn fresh_work(
    prepared: &Prepared<'_, '_>,
    values: &[f64],
    phase: Phase,
) -> TestResult<Option<u64>> {
    let mut budget = ledger()?;
    match phase {
        Phase::NativeExpm1 => {
            for &x in values {
                black_box(prepared.expm1.evaluate(
                    Expm1Input {
                        endpoints: [x, x],
                        center: x,
                        bits: 40,
                    },
                    prepared.arithmetic,
                )?);
            }
        }
        Phase::NativeExp => {
            for &x in values {
                black_box(prepared.exp.evaluate(
                    ExpInput {
                        endpoints: [x, x],
                        center: x,
                        bits: 40,
                    },
                    prepared.arithmetic,
                )?);
            }
        }
        Phase::BatchExpm1 => {
            let inputs: Vec<_> = values
                .iter()
                .map(|&x| Expm1Input {
                    endpoints: [x, x],
                    center: x,
                    bits: 40,
                })
                .collect();
            drop(black_box(prepared.expm1.evaluate_batch(
                &inputs,
                prepared.arithmetic,
                &mut budget,
            )?));
        }
        Phase::BatchExp => {
            let inputs: Vec<_> = values
                .iter()
                .map(|&x| ExpInput {
                    endpoints: [x, x],
                    center: x,
                    bits: 40,
                })
                .collect();
            drop(black_box(prepared.exp.evaluate_batch(
                &inputs,
                prepared.arithmetic,
                &mut budget,
            )?));
        }
        Phase::PlatformExpm1 => {
            for x in values {
                black_box(x.exp_m1());
            }
            return Ok(None);
        }
        Phase::PlatformExp => {
            for x in values {
                black_box(x.exp());
            }
            return Ok(None);
        }
    }
    Ok(Some(budget.usage().work))
}

/// Witness the reservation scan independently of batch input traversal.
/// # Panics
/// Panics if a single registered owner does not cost one visited entry.
fn registration_scan() -> TestResult<u64> {
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let mut budget = ledger()?;
    let arithmetic = ArithmeticContext::new()?;
    // Match the ordinary harness's cold, warm and changed-target lifecycle.
    for bits in [40, 40, 39] {
        let mut request = request.clone();
        request.bits = bits;
        let prepared = fixture.cas.prepare_expm1(
            &request,
            Policy::default(),
            &mut budget,
            &mut Counts::default(),
        )?;
        drop(prepared);
    }
    let outcome = PreparedExpm1::new(
        &mut fixture.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut budget,
        &mut Counts::default(),
    )?;
    drop(outcome);
    let before = budget.usage().work;
    fixture.cas.with_retained_graph(&mut budget, |_| {
        Ok::<_, nrz_bindings::readings::algebra::Fault>(())
    })?;
    let visited = budget.usage().work - before;
    let mut fresh = ledger()?;
    fixture.cas.with_retained_graph(&mut fresh, |_| {
        Ok::<_, nrz_bindings::readings::algebra::Fault>(())
    })?;
    assert_eq!(fresh.usage().work, 0);
    assert_eq!(visited, 1);
    Ok(visited)
}

/// Each item is the full unsorted block; fresh execution owns exactly one ledger.
/// Reset returns empty, fully reserved scratch before timing; run supplies complete results.
/// One retained adapter handles four points, a changed four-point block, then eight points.
/// Ordinary snapshots and Work agree for all six phases and every invocation.
/// The observed exact-work total also rejects rebuilding a plan during reset.
/// # Errors
/// Returns fixture, sampling or shared-guard errors.
/// # Panics
/// Panics on incomplete evidence, changed results, hidden Work or a per-point item.
/// ´claim:matrix:prepared-exponential-sampling-blocks´
/// ´test:integration:prepared-exponential-sampling-blocks´
#[test]
fn prepared_exponential_sampling_blocks() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
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
    let scan = registration_scan()?;
    let mut changed = inputs(4)?;
    changed.rotate_left(1);
    changed[0] = -0.75;
    let mut cases = Vec::new();
    for values in [inputs(4)?, changed, inputs(8)?] {
        let shape = [values.len()];
        let ordinary = measure(&values)?;
        cases.push((Block::new(values), ordinary, shape));
    }
    for phase in [
        Phase::NativeExpm1,
        Phase::NativeExp,
        Phase::BatchExpm1,
        Phase::BatchExp,
        Phase::PlatformExpm1,
        Phase::PlatformExp,
    ] {
        let mut adapter = Adapter {
            phase,
            allowances: sampling::allowances(),
            inspect: Some(inspect),
        };
        for (block, ordinary, shape) in &cases {
            exercise(&prepared, block, ordinary, *shape, scan, &mut adapter)?;
        }
    }
    Ok(())
}

/// Check every iteration against the ordinary reference and observed ledgers.
fn sample_phase(
    prepared: &Prepared<'_, '_>,
    block: &Block,
    adapter: &mut Adapter,
    shape: [usize; 1],
    fresh: Option<u64>,
) -> TestResult {
    let phase = adapter.phase;
    let mut checked = lifecycle::Checked { inner: adapter };
    let mut settings = sampling::settings()?;
    settings.warm_up = Duration::from_micros(1);
    settings.measurement = Duration::from_micros(55);
    settings.wall_budget = Duration::from_secs(100);
    settings.max_total_iterations = 100;
    settings.max_iterations_per_sample = 10;
    let clock = Clock::default();
    let (report, observed) = trace::observed(4096, || {
        sample_with_clock(
            prepared,
            &sampling::representative(block, phase, &shape),
            &mut checked,
            settings,
            &clock,
        )
    });
    let report = report?;
    assert_eq!(*report.stop(), SamplingStop::Complete, "{phase:?}");
    assert_eq!(report.elements_per_item(), Some(shape[0] as u64));
    assert_eq!(report.identity().shape, &shape);
    assert_eq!(
        report.analysis().ok_or("analysis")?.elements_per_item(),
        Some(shape[0] as u64)
    );
    let mut work = 0;
    for iteration in report.iterations() {
        let receipt = iteration.receipt.ok_or("receipt")?;
        assert_eq!(receipt.status, ReceiptStatus::Complete);
        let expected_work = fresh.map_or(Work::PlatformNoEngineWork, Work::Engine);
        assert_eq!(receipt.operation, Some(expected_work));
        assert_eq!(receipt.exit, Some(expected_work));
        assert_eq!(
            iteration.entry,
            Some(fresh.map_or(Work::PlatformNoEngineWork, |_| Work::Engine(0)))
        );
        if let Some(Work::Engine(value)) = receipt.operation {
            work += value;
        }
    }
    assert!(observed.complete);
    let observed_work: u64 = observed
        .ledgers
        .values()
        .map(|ledger| ledger.last - ledger.first)
        .sum();
    assert_eq!(observed_work, work);
    EXPECTED.with_borrow(|state| assert_eq!(state.2, report.iterations().len()));
    Ok(())
}

/// Change representatives without replacing the adapter or its immutable preparation.
fn exercise(
    prepared: &Prepared<'_, '_>,
    block: &Block,
    ordinary: &[Observation],
    shape: [usize; 1],
    scan: u64,
    adapter: &mut Adapter,
) -> TestResult {
    let phase = adapter.phase;
    let row = ordinary
        .iter()
        .find(|r| r.phase == phase.name())
        .ok_or("ordinary phase")?;
    let fresh = fresh_work(prepared, &block.values, phase)?;
    let difference = if matches!(phase, Phase::BatchExpm1 | Phase::BatchExp) {
        scan
    } else {
        0
    };
    assert_eq!(row.work, fresh.map(|work| work + difference));
    let expected: Vec<_> = row
        .results
        .iter()
        .map(|r| match r {
            Snapshot::Exp(v) => Snapshot::Exp(*v),
            Snapshot::Expm1(v) => Snapshot::Expm1(*v),
        })
        .collect();
    let platform: Vec<_> = match phase {
        Phase::PlatformExp => block.values.iter().map(|x| x.exp().to_bits()).collect(),
        Phase::PlatformExpm1 => block.values.iter().map(|x| x.exp_m1().to_bits()).collect(),
        _ => Vec::new(),
    };
    if !platform.is_empty() {
        assert_eq!(platform.iter().fold(0, |a, b| a ^ b), row.checksum);
    }
    EXPECTED.with_borrow_mut(|state| *state = (expected, platform, 0));
    sample_phase(prepared, block, adapter, shape, fresh)?;
    Ok(())
}
