// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit host export of live-clock observations; no timing is an acceptance threshold.

use super::*;
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use std::io::Write;

/// Live execution delegates the same operation under separate host cost assumptions.
///
/// Modeled zero costs belong only to the deterministic clock. The live host supplies
/// allowances covering each whole callback, including allocation and destructors;
/// additional scheduling delay remains separate from those execution assumptions.
#[derive(Debug)]
struct LiveAdapter<A> {
    /// Ordinary operation and its private resource/accounting controls.
    inner: A,
    /// Explicit nonzero live execution and mandatory finalization allowances.
    bounds: AdapterBounds,
}

impl<P: ?Sized, I: ?Sized, A: HostOperation<P, I>> HostOperation<P, I> for LiveAdapter<A> {
    type Context = A::Context;
    type Output = A::Output;
    type Error = A::Error;
    type Work = A::Work;

    fn bounds(&self) -> Option<AdapterBounds> {
        Some(self.bounds)
    }
    fn reset(
        &mut self,
        prepared: &P,
        item: &I,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Self::Context, Self::Error> {
        self.inner.reset(prepared, item, deadline)
    }
    fn snapshot(
        &mut self,
        context: &Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Option<Self::Work> {
        self.inner.snapshot(context, deadline)
    }
    fn run(
        &mut self,
        prepared: &P,
        item: &I,
        context: &mut Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Self::Output, Self::Error> {
        self.inner.run(prepared, item, context, deadline)
    }
    fn receipt(
        &mut self,
        context: &Self::Context,
        output: &Result<Self::Output, Self::Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Self::Work> {
        self.inner.receipt(context, output, deadline)
    }
    fn dispose(
        &mut self,
        output: Result<Self::Output, Self::Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Self::Error> {
        self.inner.dispose(output, deadline)
    }
    fn cleanup(
        &mut self,
        context: Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Self::Error> {
        self.inner.cleanup(context, deadline)
    }
}

/// Exercise the production `Instant` path and explicitly export its structured report.
///
/// This manual evidence run accepts either complete analysis or a structured stop;
/// it never asserts a live duration, interval width, rate or nominal coverage.
/// The host's output here is explicit and separate from the silent sampler.
/// ´test:integration:host-sampling-live-observation´
#[test]
#[ignore = "explicit host export of live observations, not a timing pin"]
fn live_observation() -> TestResult {
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let arithmetic = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut fixture.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let input = Expm1Input {
        endpoints: [-0.25, -0.25],
        center: -0.25,
        bits: 40,
    };
    let clock = Clock::default();
    let live = Cell::new(0);
    // This host assumes each complete callback, including its allocator/drop
    // work, takes at most one second under its declared execution conditions.
    let allowance = Duration::from_secs(1);
    let mut adapter = LiveAdapter {
        inner: Adapter {
            arithmetic: &arithmetic,
            clock: &clock,
            live: &live,
            charged: true,
            outputs: Vec::with_capacity(10_000),
        },
        bounds: AdapterBounds {
            uninterrupted: allowance,
            receipt: allowance,
            disposal: allowance,
            cleanup: allowance,
        },
    };
    let mut policy = settings()?;
    policy.warm_up = Duration::from_micros(10);
    policy.measurement = Duration::from_micros(100);
    policy.wall_budget = Duration::from_secs(2);
    policy.max_total_iterations = 10_000;
    policy.max_iterations_per_sample = 2000;
    policy.caps.report_bytes = 4_000_000;
    policy.bounds = SamplerBounds {
        uninterrupted: Duration::from_secs(1),
        finalization: Duration::from_secs(1),
    };
    let report = sample(
        &&plan,
        &representative(&input, true, false),
        &mut adapter,
        policy,
    )?;
    for record in report.iterations() {
        if let Some(receipt) = record.receipt {
            assert_eq!(record.entry, Some(0));
            assert_eq!(receipt.exit, Some(1));
            assert_eq!(receipt.operation, receipt.exit);
        }
    }
    assert_eq!(live.get(), 0);
    export(&report)
}

/// Explicit host output remains separate from the sampler's structured return.
fn export(report: &SamplingReport<'_, u64, Error>) -> TestResult {
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "host export: adapter_bounds={:?}; U={:?}; C={:?}",
        report.adapter_bounds(),
        report.uninterrupted_bound(),
        report.finalization_bound()
    )?;
    writeln!(
        output,
        "host export: stop={:?}; warm_up={}; rows={}; measured={:?}; total={:?}; seed={}",
        report.stop(),
        report.warm_up_iterations(),
        report.rows().len(),
        report.measured_elapsed(),
        report.total_elapsed(),
        report.settings().analysis.seed()
    )?;
    if let Some(analysis) = report.analysis() {
        writeln!(
            output,
            "host export: slope={:?}; throughput={:?}; serial_1000={:?}",
            analysis.estimates().slope,
            analysis.throughput(),
            report.extrapolate_batch(1000)?
        )?;
    }
    Ok(())
}
