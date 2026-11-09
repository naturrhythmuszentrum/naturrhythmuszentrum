// SPDX-License-Identifier: AGPL-3.0-only
//! Independent controlled execution costs, Work and live-resource inventories.

use super::*;
use std::{cell::Cell, rc::Rc, time::Duration};

/// Test time is integer milliseconds; no assertion depends on live wall time.
const fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// Shared deterministic clock with one optional injected phase cost.
#[derive(Debug, Default)]
struct Clock {
    /// Current monotonic timestamp.
    time: Cell<Duration>,
    /// One phase-specific cost, consumed on its first matching checkpoint.
    injection: Cell<Option<(SamplingPhase, Duration)>>,
}

impl Clock {
    /// Advance only the fixture's modeled execution or scheduling cost.
    fn advance(&self, value: Duration) {
        self.time.set(self.time.get() + value);
    }
}

impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        self.time.get()
    }
    fn checkpoint(&self, phase: SamplingPhase) {
        if let Some((wanted, cost)) = self.injection.get()
            && wanted == phase
        {
            self.injection.set(None);
            self.advance(cost);
        }
    }
}

/// Live private context, proving cleanup also happens on expiry/error.
#[derive(Debug)]
struct Context {
    /// Ordinary cumulative accounting owned by this iteration only.
    work: u64,
    /// Unique owner identity, never rewound by reset.
    owner: u64,
    /// Shared count of live private objects.
    live: Rc<Cell<usize>>,
}

impl Drop for Context {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

/// Ordinary output with an independently checked disposal lifetime.
#[derive(Debug)]
struct Output {
    /// Numerical result of the immutable prepared problem.
    value: u64,
    /// Operation-owned ordinary receipt created within the timed call.
    receipt: Work,
    /// Shared output inventory.
    live: Rc<Cell<usize>>,
}

impl Drop for Output {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

/// Fixed-size evidence retains owner identity and ordinary accounting separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Work {
    /// Private ledger identity.
    owner: u64,
    /// Monotone work counter.
    spent: u64,
}

/// Distinct host errors prove ordinary errors survive sampling and finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Reset,
    Operation,
    Disposal,
    Cleanup,
}

/// One omitted field despite the adapter's claimed complete coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingEvidence {
    /// Entry ledger snapshot is unavailable.
    Entry,
    /// Exit ledger snapshot is unavailable.
    Exit,
    /// Ordinary operation evidence is unavailable.
    Operation,
}

/// Adapter with independently configured sequential costs and ordinary charges.
#[derive(Debug)]
struct Operation<'a> {
    /// Shared deterministic clock.
    clock: &'a Clock,
    /// Reset/snapshot/run/receipt/disposal/cleanup costs, in invocation order.
    costs: [Duration; 6],
    /// Extra scheduling delay excluded from the declared operation execution cost.
    delay: Duration,
    /// Independently tracked live resources.
    live: Rc<Cell<usize>>,
    /// Number of fresh private owners created.
    resets: u64,
    /// Number of actual operation invocations.
    calls: u64,
    /// Last returned numerical result.
    result: Option<u64>,
    /// Optional host failure site.
    failure: Option<Failure>,
    /// Explicit receipt coverage declaration.
    complete: bool,
    /// Optional independent omission, without changing the claimed coverage status.
    missing: Option<MissingEvidence>,
    /// Whether the host can attest cooperative/isolation bounds.
    supported: bool,
}

impl<'a> Operation<'a> {
    /// An operation takes one millisecond and charges exactly seven Work units.
    fn new(clock: &'a Clock) -> Self {
        Self {
            clock,
            costs: [
                Duration::ZERO,
                Duration::ZERO,
                ms(1),
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
            ],
            delay: Duration::ZERO,
            live: Rc::new(Cell::new(0)),
            resets: 0,
            calls: 0,
            result: None,
            failure: None,
            complete: true,
            missing: None,
            supported: true,
        }
    }
}

impl HostOperation<[u64; 2], u64> for Operation<'_> {
    type Context = Context;
    type Output = Output;
    type Error = Failure;
    type Work = Work;
    fn bounds(&self) -> Option<AdapterBounds> {
        self.supported.then_some(AdapterBounds {
            uninterrupted: self.costs[0].max(self.costs[1]).max(self.costs[2]),
            receipt: self.costs[3],
            disposal: self.costs[4],
            cleanup: self.costs[5],
        })
    }
    fn reset(
        &mut self,
        _: &[u64; 2],
        _: &u64,
        _: &SamplingDeadline<'_>,
    ) -> Result<Context, Failure> {
        self.clock.advance(self.costs[0]);
        if self.failure == Some(Failure::Reset) {
            return Err(Failure::Reset);
        }
        self.resets += 1;
        self.live.set(self.live.get() + 1);
        Ok(Context {
            work: 0,
            owner: self.resets,
            live: self.live.clone(),
        })
    }
    fn snapshot(&mut self, context: &Context, _: &SamplingDeadline<'_>) -> Option<Work> {
        self.clock.advance(self.costs[1]);
        (self.missing != Some(MissingEvidence::Entry)).then_some(Work {
            owner: context.owner,
            spent: context.work,
        })
    }
    fn run(
        &mut self,
        prepared: &[u64; 2],
        item: &u64,
        context: &mut Context,
        _: &SamplingDeadline<'_>,
    ) -> Result<Output, Failure> {
        self.calls += 1;
        self.clock.advance(self.delay + self.costs[2]);
        context.work += 7;
        if self.failure == Some(Failure::Operation) {
            return Err(Failure::Operation);
        }
        self.live.set(self.live.get() + 1);
        Ok(Output {
            value: (prepared[0] + prepared[1]) * item,
            receipt: Work {
                owner: context.owner,
                spent: context.work,
            },
            live: self.live.clone(),
        })
    }
    fn receipt(
        &mut self,
        context: &Context,
        output: &Result<Output, Failure>,
        _: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Work> {
        let exit = Work {
            owner: context.owner,
            spent: context.work,
        };
        self.clock.advance(self.costs[3]);
        WorkReceipt {
            exit: (self.missing != Some(MissingEvidence::Exit)).then_some(exit),
            operation: (self.missing != Some(MissingEvidence::Operation))
                .then_some(output.as_ref().map_or(exit, |o| o.receipt)),
            status: if self.complete {
                ReceiptStatus::Complete
            } else {
                ReceiptStatus::Incomplete
            },
        }
    }
    fn dispose(
        &mut self,
        output: Result<Output, Failure>,
        _: &SamplingDeadline<'_>,
    ) -> Result<(), Failure> {
        self.clock.advance(self.costs[4]);
        if let Ok(output) = output {
            self.result = Some(output.value);
        }
        if self.failure == Some(Failure::Disposal) {
            Err(Failure::Disposal)
        } else {
            Ok(())
        }
    }
    fn cleanup(&mut self, context: Context, _: &SamplingDeadline<'_>) -> Result<(), Failure> {
        self.clock.advance(self.costs[5]);
        drop(context);
        if self.failure == Some(Failure::Cleanup) {
            Err(Failure::Cleanup)
        } else {
            Ok(())
        }
    }
}

/// Explicit ample fixture policy; the sampler provides no defaults.
fn settings() -> SamplingSettings {
    let caps = AnalysisCaps {
        report_bytes: 1_000_000,
        scratch_bytes: 1_000_000,
    };
    SamplingSettings {
        analysis: AnalysisSettings::new(10, 1000, 0.95, 42, caps).expect("valid policy"),
        warm_up: ms(1),
        measurement: ms(55),
        wall_budget: ms(1000),
        max_total_iterations: 1000,
        max_iterations_per_sample: 100,
        caps: AnalysisCaps {
            report_bytes: 2_000_000,
            scratch_bytes: 1_000_000,
        },
        bounds: SamplerBounds {
            uninterrupted: Duration::ZERO,
            finalization: Duration::ZERO,
        },
    }
}

/// Named scalar representative, with explicit element meaning and scope.
const fn representative() -> Representative<'static, u64> {
    Representative {
        item: &3,
        elements_per_item: Some(2),
        identity: SamplingIdentity {
            representative: "three",
            shape: &[2],
            operation_revision: "sum-v1",
            workload: "pair",
            scope: "prepared pair sum times item",
            trace_configuration: "disabled",
        },
    }
}

/// A complete known-Work run returns both throughput units and serial batch uncertainty.
/// ´test:unit:sampling-collection-known-work´
#[test]
fn known_work_and_repeat() -> Result<(), SamplingError> {
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    let prepared = [4, 5];
    let before = prepared;
    for _ in 0..2 {
        let report = sample_with_clock(
            &prepared,
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(report.stop(), &SamplingStop::Complete);
        assert_eq!(report.warm_up_iterations(), 1);
        assert_eq!(report.linear_step(), Some(1));
        assert_eq!(report.rows().len(), 10);
        assert_eq!(report.iterations().len(), 56);
        assert_eq!(report.measured_elapsed(), ms(56));
        assert_eq!(report.total_elapsed(), ms(56));
        let analysis = report.analysis().expect("complete estimates");
        assert!((analysis.estimates().slope.point_estimate - 0.001).abs() < 1e-15);
        let rates = analysis.throughput();
        assert!((rates.items_per_second.point_estimate - 1000.0).abs() < 1e-9);
        assert!(
            (rates.elements_per_second.expect("elements").point_estimate - 2000.0).abs() < 1e-9
        );
        let batch = report
            .extrapolate_batch(500)
            .expect("finite")
            .expect("complete");
        assert!((batch.estimated_seconds - 0.5).abs() < 1e-12);
        for record in report.iterations() {
            let entry = record.entry.expect("entry");
            let receipt = record.receipt.expect("receipt");
            assert_eq!(entry.spent, 0);
            assert_eq!(
                receipt.exit,
                Some(Work {
                    owner: entry.owner,
                    spent: 7
                })
            );
            assert_eq!(receipt.operation, receipt.exit);
            assert_eq!(receipt.status, ReceiptStatus::Complete);
        }
        assert_eq!(operation.live.get(), 0);
        assert_eq!(operation.result, Some(27));
        assert_eq!(prepared, before);
    }
    assert_eq!(operation.resets, 112);
    Ok(())
}

/// Exact schedule ratios pin geometric groups and ceil behavior without float rounding.
/// ´test:unit:sampling-linear-schedule-caps´
#[test]
fn schedule_caps() -> Result<(), SamplingError> {
    let mut policy = settings();
    assert_eq!(schedule::linear_step(policy, 1, ms(1))?, 1);
    policy.measurement += Duration::from_nanos(1);
    assert_eq!(schedule::linear_step(policy, 1, ms(1))?, 2);
    policy.max_total_iterations = 110;
    assert_eq!(
        schedule::linear_step(policy, 1, ms(1)),
        Err(SamplingError::IterationCap)
    );
    assert_eq!(schedule::next_group(8, 15, settings())?, 16);
    assert_eq!(
        schedule::next_group(64, 127, settings()),
        Err(SamplingError::IterationCap)
    );
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    policy = settings();
    policy.warm_up = ms(5);
    let report = sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock)?;
    assert_eq!(report.warm_up_iterations(), 7);
    assert_eq!(report.warm_up_elapsed(), ms(7));
    let groups: Vec<_> = report
        .iterations()
        .iter()
        .take(7)
        .map(|r| r.position)
        .collect();
    assert_eq!(
        groups,
        [
            SchedulePosition::WarmUp(1),
            SchedulePosition::WarmUp(2),
            SchedulePosition::WarmUp(2),
            SchedulePosition::WarmUp(4),
            SchedulePosition::WarmUp(4),
            SchedulePosition::WarmUp(4),
            SchedulePosition::WarmUp(4)
        ]
    );
    Ok(())
}

#[path = "tests_deadline.rs"]
mod deadlines;
#[path = "tests_failures.rs"]
mod failures;
