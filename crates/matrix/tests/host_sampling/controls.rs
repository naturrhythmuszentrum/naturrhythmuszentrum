// SPDX-License-Identifier: AGPL-3.0-only
//! Same prepared native problem with zero/charged Work and scoped tracing.

use super::*;
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tracing::{
    Event, Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

/// Local subscriber stores only bounded atomic counters, never files or stdout.
#[derive(Clone, Debug, Default)]
struct Observer {
    /// Next observed span identity.
    spans: Arc<AtomicU64>,
    /// Native kernel spans prove the enabled path was exercised.
    kernels: Arc<AtomicU64>,
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
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// The same retained graph/plan/input survives two calls, tracing, charging and failure.
/// ´test:integration:host-sampling-prepared-native-work´
#[test]
fn prepared_native_work() -> TestResult {
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let arithmetic = ArithmeticContext::new()?;
    // Prepare once to establish a retained-graph baseline before taking the
    // owner's exclusive construction borrow. All sampling reuses the next owner.
    drop(PreparedExpm1::new(
        &mut fixture.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut ledger()?,
        &mut Counts::default(),
    )?);
    let graph = fixture.cas.canonical_snapshot(&mut ledger()?)?;
    let inventory = (
        fixture.cas.counts(),
        fixture.cas.retained_judgment_count(),
        fixture.cas.retained_judgment_bytes(),
    );
    let mut counts = Counts::default();
    let mut preparation = ledger()?;
    let plan = PreparedExpm1::new(
        &mut fixture.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut preparation,
        &mut counts,
    )?;
    let input = Expm1Input {
        endpoints: [-0.25, -0.25],
        center: -0.25,
        bits: 40,
    };
    let expected = bits(plan.evaluate(input, &arithmetic)?.reading());
    let retained = format!("{plan:?}");
    let authority = plan.authority().statement_key().to_vec();
    let proof = format!("{:?}", plan.authority().proof());
    let owned = plan.owned_bytes();
    let usage = preparation.usage();
    let before_counts = counts.clone();
    exercise_plan(&plan, &arithmetic, &input, expected)?;
    assert_eq!(format!("{plan:?}"), retained);
    assert_eq!(plan.authority().statement_key(), authority);
    assert_eq!(format!("{:?}", plan.authority().proof()), proof);
    assert_eq!(plan.owned_bytes(), owned);
    drop(plan);
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, graph);
    assert_eq!(
        (
            fixture.cas.counts(),
            fixture.cas.retained_judgment_count(),
            fixture.cas.retained_judgment_bytes()
        ),
        inventory
    );
    assert_eq!(preparation.usage(), usage);
    assert_eq!(counts, before_counts);
    assert_eq!(
        input.endpoints.map(f64::to_bits),
        [(-0.25_f64).to_bits(); 2]
    );
    Ok(())
}

/// Scoped tracing and sampler calls preserve process state and every numerical result.
fn exercise_plan(
    plan: &PreparedExpm1<'_>,
    arithmetic: &ArithmeticContext,
    input: &Expm1Input,
    expected: [u64; 5],
) -> TestResult {
    let cwd = std::env::current_dir()?;
    let environment: std::collections::BTreeMap<_, _> = std::env::vars_os().collect();
    let observer = Observer::default();
    for charged in [false, true] {
        let disabled = run_twice(plan, arithmetic, input, charged, false, expected)?;
        let enabled = tracing::subscriber::with_default(observer.clone(), || {
            run_twice(plan, arithmetic, input, charged, true, expected)
        })?;
        assert_eq!(disabled, enabled);
    }
    assert!(observer.kernels.load(Ordering::Relaxed) > 0);
    assert!(!tracing::dispatcher::get_default(
        tracing::Dispatch::is::<Observer>
    ));
    assert_eq!(std::env::current_dir()?, cwd);
    assert_eq!(
        std::env::vars_os().collect::<std::collections::BTreeMap<_, _>>(),
        environment
    );
    Ok(())
}

/// Use the public object-safe adapter surface and compare every receipt to ordinary Work.
fn run_twice(
    plan: &PreparedExpm1<'_>,
    arithmetic: &ArithmeticContext,
    input: &Expm1Input,
    charged: bool,
    observed: bool,
    expected: [u64; 5],
) -> TestResult<Vec<u64>> {
    let clock = Clock::default();
    let live = Cell::new(0);
    let mut adapter = Adapter {
        arithmetic,
        clock: &clock,
        live: &live,
        charged,
        outputs: Vec::with_capacity(112),
    };
    let mut work = Vec::new();
    for _ in 0..2 {
        let erased: &mut dyn HostOperation<
            &PreparedExpm1<'_>,
            Expm1Input,
            Context = Context<'_>,
            Output = Output<'_>,
            Error = Error,
            Work = u64,
        > = &mut adapter;
        let representative = representative(input, charged, observed);
        let report = sample_with_clock(&plan, &representative, erased, settings()?, &clock)?;
        assert_eq!(report.stop(), &SamplingStop::Complete);
        assert_eq!(
            report.identity().trace_configuration,
            representative.identity.trace_configuration
        );
        assert_eq!(report.iterations().len(), 56);
        for record in report.iterations() {
            assert_eq!(record.entry, Some(0));
            let receipt = record.receipt.expect("ordinary receipt");
            assert_eq!(receipt.exit, Some(u64::from(charged)));
            assert_eq!(receipt.operation, receipt.exit);
            assert_eq!(receipt.status, ReceiptStatus::Complete);
            work.push(receipt.operation.expect("complete"));
        }
        assert_eq!(live.get(), 0);
        assert!(report.extrapolate_batch(100)?.is_some());
    }
    assert_eq!(adapter.outputs, vec![expected; 112]);
    // Native refusal still preserves zero exact Work and disposes its fresh ledger.
    let invalid = Expm1Input {
        endpoints: [1.0, 1.0],
        center: 1.0,
        bits: 40,
    };
    adapter.charged = false;
    let report = sample_with_clock(
        &plan,
        &representative(&invalid, false, observed),
        &mut adapter,
        settings()?,
        &clock,
    )?;
    assert_eq!(
        report.stop(),
        &SamplingStop::HostError(
            SamplingPhase::Operation,
            Error::Native(Expm1Failure::OutsideDomain)
        )
    );
    assert_eq!(
        report.iterations()[0]
            .receipt
            .expect("failure receipt")
            .operation,
        Some(0)
    );
    assert_eq!(live.get(), 0);
    Ok(work)
}
