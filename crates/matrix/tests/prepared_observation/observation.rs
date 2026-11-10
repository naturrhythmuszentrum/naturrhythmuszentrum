// SPDX-License-Identifier: AGPL-3.0-only
//! Route-complete native reuse observation without an unbounded event buffer.
use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tracing::{
    Event, Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

/// Shared interest-rebuild guard for every subscriber in this integration target.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the shared dispatcher guard remains private to this integration target"
)]
pub(super) static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Exp composition reuses authority; a spent ledger never enters either kernel span.
/// Three spans is a regression pin for three exp calls without discarded basis calls, not a Work tariff.
/// # Panics
/// Panics if native reuse emits construction events or unpaid traversal reaches a kernel.
/// ´claim:matrix:prepared-exp-observed-reuse´
/// ´test:integration:prepared-exp-observed-reuse´
#[test]
fn prepared_exp_observed_reuse() -> TestResult {
    use nrz_matrix::PreparedExp;
    use nrz_scalar::ExpInput;
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    let observer = Observer::default();
    tracing::subscriber::with_default(observer.clone(), || -> TestResult {
        let mut f = Fixture::new()?;
        let r = broad(&mut f, 40)?;
        let context = ArithmeticContext::new()?;
        let mut counts = Counts::default();
        let plan = PreparedExp::new(
            &mut f.cas,
            &r,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut counts,
        )
        .map_err(|e| e.to_string())?;
        let before = counts.clone();
        let events = observer.events.load(Ordering::Relaxed);
        let constructions = observer.constructions.load(Ordering::Relaxed);
        assert!(constructions > 1);
        let point = |x| ExpInput {
            endpoints: [x, x],
            center: x,
            bits: 40,
        };
        for x in [-0.25, -1.0, -0.5] {
            plan.evaluate(point(x), &context)?;
        }
        assert_eq!(counts, before);
        assert_eq!(observer.events.load(Ordering::Relaxed), events);
        assert_eq!(
            observer.constructions.load(Ordering::Relaxed),
            constructions
        );
        let kernels = observer.kernels.load(Ordering::Relaxed);
        assert_eq!(kernels, 3);
        let mut empty = Budget::new(Limits {
            work: 7,
            ..Limits::default()
        })?;
        empty.work(7, nrz_bindings::readings::algebra::Location::new("caller"))?;
        assert_eq!(
            plan.evaluate_batch(&[point(-0.125)], &context, &mut empty)
                .expect_err("prepaid walk")
                .kind,
            FaultKind::Exhausted {
                reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                budget: BudgetKind::Work,
                limit: 7,
                needed: 8
            }
        );
        assert_eq!(empty.usage().work, 7);
        assert_eq!(observer.kernels.load(Ordering::Relaxed), kernels);
        Ok(())
    })
}

/// Fixed counters distinguish an observed native call from an empty event stream.
#[derive(Clone, Debug, Default)]
struct Observer {
    /// Monotone bounded-test span identity counter.
    spans: Arc<AtomicU64>,
    /// Actual kernel spans.
    kernels: Arc<AtomicU64>,
    /// Construction events across both CAS preparation and native lowering.
    constructions: Arc<AtomicU64>,
    /// Native program construction events for prepared expm1 alone.
    expm1_programs: Arc<AtomicU64>,
    /// All exact/other events, catching hidden nested receipt traffic.
    events: Arc<AtomicU64>,
}

impl Subscriber for Observer {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, span: &Attributes<'_>) -> Id {
        if span.metadata().name() == "nrz.kernel" {
            self.kernels.fetch_add(1, Ordering::Relaxed);
        }
        Id::from_u64(self.spans.fetch_add(1, Ordering::Relaxed) + 1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        self.events.fetch_add(1, Ordering::Relaxed);
        if event.metadata().name() == "nrz.construct" {
            self.constructions.fetch_add(1, Ordering::Relaxed);
            let mut fields = Expm1Construction::default();
            event.record(&mut fields);
            if fields.prepared_expm1 && fields.native_program {
                self.expm1_programs.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// Select the existing native-lowering event independently of CAS construction.
#[derive(Default)]
struct Expm1Construction {
    /// The construction belongs to prepared expm1.
    prepared_expm1: bool,
    /// The construction creates the native program.
    native_program: bool,
}

impl tracing::field::Visit for Expm1Construction {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "family" => self.prepared_expm1 = value == "prepared-expm1",
            "operation" => self.native_program = value == "native program",
            _ => {}
        }
    }

    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

/// Observed preparation builds authority once; three native calls build nothing.
/// # Errors
/// Returns fixture, preparation or numerical failures.
/// # Panics
/// Panics if observation loses native coverage or detects exact work/construction in reuse.
/// ´claim:matrix:prepared-expm1-observed-reuse´
/// ´test:integration:prepared-expm1-observed-reuse´
#[test]
fn prepared_expm1_observed_reuse() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    let observer = Observer::default();
    tracing::subscriber::with_default(observer.clone(), || -> TestResult {
        let mut f = Fixture::new()?;
        let r = broad(&mut f, 40)?;
        let context = ArithmeticContext::new()?;
        let mut counts = Counts::default();
        let mut budget = ledger()?;
        let plan = PreparedExpm1::new(
            &mut f.cas,
            &r,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut budget,
            &mut counts,
        )?;
        let events = observer.events.load(Ordering::Relaxed);
        let constructions = observer.constructions.load(Ordering::Relaxed);
        assert!(constructions > 1);
        assert!(counts.checks > 0);
        let before = counts.clone();
        let work = budget.usage().work;
        let mut outputs = Vec::new();
        for x in [-0.25, -1.0, -0.5] {
            outputs.push(plan.evaluate(point(x), &context)?.reading());
        }
        assert_eq!(observer.kernels.load(Ordering::Relaxed), 3);
        assert_eq!(
            observer.constructions.load(Ordering::Relaxed),
            constructions
        );
        assert_eq!(observer.events.load(Ordering::Relaxed), events);
        assert_eq!(counts, before);
        assert_eq!(budget.usage().work, work);
        let mut empty = Budget::new(Limits {
            work: 0,
            ..Limits::default()
        })?;
        let error = plan
            .evaluate_batch(&[point(-0.125)], &context, &mut empty)
            .expect_err("prepaid walk");
        assert_eq!(
            error.kind,
            FaultKind::Exhausted {
                reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                budget: BudgetKind::Work,
                limit: 0,
                needed: 1,
            }
        );
        assert_eq!(observer.kernels.load(Ordering::Relaxed), 3);
        tracing::subscriber::with_default(
            tracing::subscriber::NoSubscriber::default(),
            || -> TestResult {
                for (x, observed) in [-0.25, -1.0, -0.5].into_iter().zip(outputs) {
                    let unobserved = plan.evaluate(point(x), &context)?.reading();
                    assert_eq!(
                        observed.endpoints.map(f64::to_bits),
                        unobserved.endpoints.map(f64::to_bits)
                    );
                    assert_eq!(observed.center.to_bits(), unobserved.center.to_bits());
                }
                Ok(())
            },
        )?;
        Ok(())
    })
}

/// Both cold and warm owned preparation lower once; native reuse never enters CAS or construction.
#[test]
fn retained_expm1_observed_preparation_and_reuse() -> TestResult {
    use nrz_matrix::{ExecutionAccount, Heap, HeapLimits};
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    for warm in [false, true] {
        let observer = Observer::default();
        tracing::subscriber::with_default(observer.clone(), || -> TestResult {
            let mut fixture = Fixture::new()?;
            let request = broad(&mut fixture, 40)?;
            let context = ArithmeticContext::new()?;
            let mut account = ExecutionAccount::new(ledger()?)?;
            if warm {
                fixture.cas.prepare_expm1(
                    &request,
                    Policy::default(),
                    &mut ledger()?,
                    &mut Counts::default(),
                )?;
            }
            let mut heap = Heap::new(HeapLimits::default())?;
            let programs = observer.expm1_programs.load(Ordering::Relaxed);
            let mut counts = Counts::default();
            let law = heap
                .prepare_retained_expm1(
                    fixture.cas,
                    &request,
                    [-2.0, 0.0],
                    Policy::default(),
                    &context,
                    &mut counts,
                    &mut account,
                )
                .map_err(|(error, _)| error)?;
            assert_eq!(
                observer.expm1_programs.load(Ordering::Relaxed),
                programs + 1
            );
            assert_eq!(counts.preparations, 1);
            assert_eq!(counts.lookups, 1);
            if warm {
                assert_eq!(counts.proposals, 0);
                assert_eq!(counts.checks, 0);
            } else {
                assert!(counts.checks > 0);
            }
            let programs = observer.expm1_programs.load(Ordering::Relaxed);
            let constructions = observer.constructions.load(Ordering::Relaxed);
            let other = heap.retain_retained_expm1(&law, &mut account)?;
            assert_eq!(observer.expm1_programs.load(Ordering::Relaxed), programs);
            assert_eq!(
                observer.constructions.load(Ordering::Relaxed),
                constructions
            );
            drop(law);
            let events = observer.events.load(Ordering::Relaxed);
            let work = account.usage().work;
            let before = counts.clone();
            for value in [-2.0, -0.25, 0.0] {
                other.evaluate(point(value), &context)?;
            }
            assert_eq!(observer.kernels.load(Ordering::Relaxed), 3);
            assert_eq!(observer.expm1_programs.load(Ordering::Relaxed), programs);
            assert_eq!(
                observer.constructions.load(Ordering::Relaxed),
                constructions
            );
            assert_eq!(observer.events.load(Ordering::Relaxed), events);
            assert_eq!(account.usage().work, work);
            assert_eq!(counts, before);
            Ok(())
        })?;
    }
    Ok(())
}
