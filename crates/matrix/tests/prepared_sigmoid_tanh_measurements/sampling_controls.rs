// SPDX-License-Identifier: AGPL-3.0-only
//! Real block adapters under deterministic clocks and independently observed Work.
use super::*;
use nrz_matrix::sampling::{ReceiptStatus, SamplingClock, SamplingStop, sample_with_clock};
use nrz_scalar::{ArithmeticContext, SigmoidInput, TanhInput};
use sampling::{Adapter, Block, Phase, Prepared, Work};
use std::{cell::Cell, time::Duration};
use support::{Fixture, ledger};
#[path = "sampling_lifecycle.rs"]
mod lifecycle;
/// Each sampling call starts with its own empty arithmetic inventory.
fn begin_call() {
    sampling::begin_call();
    assert!(
        sampling::EVALUATIONS.with(|events| events.borrow().is_empty()),
        "sampling call clears prior arithmetic events"
    );
}
/// Every clock reading advances without a wall-time threshold.
#[derive(Debug, Default)]
struct Clock(Cell<u64>);
impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        let n = self.0.get();
        self.0.set(n + 1);
        Duration::from_micros(n)
    }
}
/// Ordinary batch and native routes use exactly the same fresh ledger state.
fn fresh_work(
    prepared: &Prepared<'_, '_>,
    values: &[f64],
    phase: Phase,
) -> TestResult<Option<u64>> {
    let mut budget = ledger()?;
    match phase {
        Phase::BatchSigmoid => {
            let v = values
                .iter()
                .map(|x| SigmoidInput {
                    endpoints: [2.0 * x, 2.0 * x],
                    center: 2.0 * x,
                    bits: 40,
                })
                .collect::<Vec<_>>();
            drop(
                prepared
                    .sigmoid
                    .evaluate_batch(&v, prepared.arithmetic, &mut budget)?,
            );
        }
        Phase::BatchTanh => {
            let v = values
                .iter()
                .map(|x| TanhInput {
                    endpoints: [*x, *x],
                    center: *x,
                    bits: 40,
                })
                .collect::<Vec<_>>();
            drop(
                prepared
                    .tanh
                    .evaluate_batch(&v, prepared.arithmetic, &mut budget)?,
            );
        }
        Phase::NativeSigmoid => {
            for x in values {
                std::hint::black_box(prepared.sigmoid.evaluate(
                    SigmoidInput {
                        endpoints: [2.0 * x, 2.0 * x],
                        center: 2.0 * x,
                        bits: 40,
                    },
                    prepared.arithmetic,
                )?);
            }
        }
        Phase::NativeTanh => {
            for x in values {
                std::hint::black_box(prepared.tanh.evaluate(
                    TanhInput {
                        endpoints: [*x, *x],
                        center: *x,
                        bits: 40,
                    },
                    prepared.arithmetic,
                )?);
            }
        }
        Phase::PlatformSigmoid | Phase::PlatformTanh => return Ok(None),
    }
    Ok(Some(budget.usage().work))
}
/// Each real ledger incarnation is reconciled; consumption and output lifetime are checked at callbacks.
/// # Errors
/// Returns fixture, sampling, or shared subscriber-guard errors.
/// # Panics
/// Panics if a callback crosses its boundary, a block changes, or Work evidence is incomplete.
/// ´claim:matrix:prepared-sigmoid-tanh-sampling-blocks´
/// ´test:integration:prepared-sigmoid-tanh-sampling-blocks´
#[test]
fn prepared_sigmoid_tanh_sampling_blocks() -> TestResult {
    let _guard = tests::DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let arithmetic = ArithmeticContext::new()?;
    let mut left = Fixture::new()?;
    let mut right = Fixture::new()?;
    let left_request = support::original(&mut left, false)?;
    let right_request = support::original(&mut right, true)?;
    let mut left_budget = ledger()?;
    let mut right_budget = ledger()?;
    let (owners, setup_trace) = trace::observed(8192, || -> TestResult<_> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-setup").entered();
        Ok((
            setup::sigmoid(&mut left, &left_request, &arithmetic, &mut left_budget)?,
            setup::tanh(&mut right, &right_request, &arithmetic, &mut right_budget)?,
        ))
    });
    let (sigmoid, tanh) = owners?;
    let prepared = Prepared {
        sigmoid: &sigmoid,
        tanh: &tanh,
        arithmetic: &arithmetic,
    };
    assert!(setup_trace.complete, "setup observation complete");
    assert!(
        !setup_trace
            .clocks
            .keys()
            .any(|(_, name, family)| name == "nrz.validation"
                && matches!(family.as_str(), "prepared-sigmoid" | "prepared-tanh")),
        "setup must not evaluate a block"
    );
    let mut changed = inputs(4)?;
    changed.rotate_left(1);
    changed[0] = 0.625;
    let mut cases = Vec::new();
    for values in [inputs(4)?, changed, inputs(8)?] {
        let count = u32::try_from(values.len())?;
        let ordinary = run(count, false, 4096)?.0;
        // The changed representative is evaluated directly by the same ordinary phases.
        let ordinary = if values == inputs(count)? {
            ordinary
        } else {
            let mut rows = phases::sigmoid(&values)?;
            rows.extend(phases::tanh(&values)?);
            rows
        };
        cases.push((Block::new(values), ordinary));
    }
    for phase in [
        Phase::NativeSigmoid,
        Phase::NativeTanh,
        Phase::BatchSigmoid,
        Phase::BatchTanh,
        Phase::PlatformSigmoid,
        Phase::PlatformTanh,
    ] {
        let mut adapter = Adapter {
            phase,
            allowances: sampling::allowances(),
        };
        for (block, ordinary) in &cases {
            exercise(&prepared, block, ordinary, &mut adapter)?;
        }
    }
    drop(sigmoid);
    drop(tanh);
    for (fixture, budget) in [(&left, &mut left_budget), (&right, &mut right_budget)] {
        let before = budget.usage().work;
        fixture.cas.with_retained_graph(budget, |_| {
            Ok::<_, nrz_bindings::readings::algebra::Fault>(())
        })?;
        assert_eq!(
            budget.usage().work - before,
            1,
            "one retained registration scan on the preparation ledger"
        );
        let mut fresh = ledger()?;
        fixture.cas.with_retained_graph(&mut fresh, |_| {
            Ok::<_, nrz_bindings::readings::algebra::Fault>(())
        })?;
        assert_eq!(
            fresh.usage().work,
            0,
            "fresh registration inventory is empty"
        );
    }
    Ok(())
}
/// Match complete ordinary snapshots and every iteration's private ledger.
fn exercise(
    prepared: &Prepared<'_, '_>,
    block: &Block,
    ordinary: &[Observation],
    adapter: &mut Adapter,
) -> TestResult {
    let phase = adapter.phase;
    let family = if matches!(
        phase,
        Phase::NativeSigmoid | Phase::BatchSigmoid | Phase::PlatformSigmoid
    ) {
        "sigmoid"
    } else {
        "tanh"
    };
    let kind = match phase {
        Phase::NativeSigmoid | Phase::NativeTanh => "native",
        Phase::BatchSigmoid | Phase::BatchTanh => "read",
        _ => "uncertified",
    };
    let row = ordinary
        .iter()
        .find(|r| r.phase == format!("{kind}-{family}"))
        .ok_or("ordinary reading")?;
    let baseline = row
        .results
        .iter()
        .filter_map(|r| {
            if let Snapshot::Platform(bits) = r {
                Some(*bits)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let expected = if baseline.is_empty() {
        row.results.clone()
    } else {
        Vec::new()
    };
    let fresh = fresh_work(prepared, &block.values, phase)?;
    if matches!(phase, Phase::BatchSigmoid | Phase::BatchTanh) {
        let batch = ordinary
            .iter()
            .find(|r| r.phase == format!("batch-{family}"))
            .ok_or("ordinary batch")?;
        assert_eq!(fresh, Some(block.values.len() as u64));
        assert_eq!(
            batch.work,
            fresh.map(|w| w + 1),
            "only the independently witnessed registration scan differs"
        );
    }
    ordinary_work(phase, row, fresh);
    let mut checked = lifecycle::Checked {
        inner: adapter,
        expected: &expected,
        baseline: &baseline,
    };
    let mut settings = sampling::settings()?;
    settings.warm_up = Duration::from_micros(1);
    settings.measurement = Duration::from_micros(55);
    settings.wall_budget = Duration::from_secs(100);
    settings.max_total_iterations = 100;
    settings.max_iterations_per_sample = 10;
    let shape = [block.values.len()];
    begin_call();
    let disabled =
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            sample_with_clock(
                prepared,
                &sampling::representative(block, phase, &shape),
                &mut checked,
                settings,
                &Clock::default(),
            )
        })?;
    begin_call();
    let clock = Clock::default();
    let (report, observed) = trace::observed(8192, || {
        sample_with_clock(
            prepared,
            &sampling::representative(block, phase, &shape),
            &mut checked,
            settings,
            &clock,
        )
    });
    let report = report?;
    assert_eq!(
        disabled.iterations(),
        report.iterations(),
        "observation preserves every iteration outcome and receipt"
    );
    assert_eq!(
        disabled.rows(),
        report.rows(),
        "observation preserves complete deterministic sample rows"
    );
    evidence(&report, &observed, fresh, block)
}

/// Independently reconcile complete receipts, owner lifetimes and callback attribution.
fn evidence(
    report: &nrz_matrix::sampling::SamplingReport<'_, Work, sampling::Error>,
    observed: &trace::Report,
    fresh: Option<u64>,
    block: &Block,
) -> TestResult {
    assert_eq!(*report.stop(), SamplingStop::Complete);
    assert_eq!(report.elements_per_item(), Some(block.values.len() as u64));
    assert_eq!(
        report.analysis().ok_or("analysis")?.elements_per_item(),
        Some(block.values.len() as u64)
    );
    assert!(observed.complete);
    let work = fresh.map_or(Work::PlatformNoEngineWork, Work::Engine);
    for iteration in report.iterations() {
        let receipt = iteration.receipt.ok_or("receipt")?;
        assert_eq!(receipt.status, ReceiptStatus::Complete);
        assert_eq!(receipt.operation, Some(work));
        assert_eq!(receipt.exit, Some(work));
        assert_eq!(
            iteration.entry,
            Some(fresh.map_or(Work::PlatformNoEngineWork, |_| Work::Engine(0)))
        );
    }
    let observed_work: u64 = observed
        .ledgers
        .values()
        .map(|ledger| ledger.last - ledger.first)
        .sum();
    assert_eq!(
        observed_work,
        fresh.unwrap_or(0) * report.iterations().len() as u64,
        "every private ledger is reconciled"
    );
    assert_eq!(
        observed
            .ledgers
            .values()
            .filter(|ledger| ledger.created && ledger.closed)
            .count(),
        if fresh.is_some() {
            report.iterations().len()
        } else {
            0
        },
        "reset creates and cleanup closes one fresh ledger per iteration"
    );
    for ((callback, name, family), (_, count)) in &observed.clocks {
        assert_ne!(
            name, "nrz.import",
            "collection must not rebuild the prepared plan"
        );
        if name == "nrz.validation"
            && matches!(family.as_str(), "prepared-sigmoid" | "prepared-tanh")
        {
            assert_eq!(callback, "sampler-run", "evaluation belongs only to run");
            assert_eq!(
                *count,
                block.values.len() as u64 * report.iterations().len() as u64,
                "whole block evaluated once per iteration"
            );
        }
    }
    Ok(())
}

/// Ordinary rows independently establish native zero and platform non-participation.
fn ordinary_work(phase: Phase, row: &Observation, fresh: Option<u64>) {
    if matches!(phase, Phase::NativeSigmoid | Phase::NativeTanh) {
        assert_eq!(row.work, Some(0), "ordinary native row records zero Work");
        assert_eq!(fresh, row.work, "native Work agrees with the ordinary row");
    }
    if matches!(phase, Phase::PlatformSigmoid | Phase::PlatformTanh) {
        assert_eq!(
            row.work, None,
            "ordinary platform row declares no engine Work"
        );
        assert_eq!(
            fresh, row.work,
            "platform Work agrees with the ordinary row"
        );
    }
}
