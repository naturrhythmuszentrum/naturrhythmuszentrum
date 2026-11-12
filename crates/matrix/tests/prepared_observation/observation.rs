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
    /// Exact ledger lifecycle, phase and construction events only.
    exact_events: Arc<AtomicU64>,
    /// Formatted supplied operands, coordinates, recursive owners and tanh outputs.
    formatted: Arc<Mutex<[usize; 4]>>,
}

impl Subscriber for Observer {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, span: &Attributes<'_>) -> Id {
        span.record(&mut FormattingVisitor(self.formatted.clone()));
        if span.metadata().name() == "nrz.kernel" {
            self.kernels.fetch_add(1, Ordering::Relaxed);
        }
        Id::from_u64(self.spans.fetch_add(1, Ordering::Relaxed) + 1)
    }
    fn record(&self, _: &Id, record: &Record<'_>) {
        record.record(&mut FormattingVisitor(self.formatted.clone()));
    }
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        event.record(&mut FormattingVisitor(self.formatted.clone()));
        self.events.fetch_add(1, Ordering::Relaxed);
        if event.metadata().target() == "nrz::work" {
            self.exact_events.fetch_add(1, Ordering::Relaxed);
        }
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

/// A field-formatting subscriber sees no supplied records on count or Work refusal, no recursive owner, and no exact event in native reuse.
/// # Panics
/// Panics if native reuse touches an exact ledger or unpaid batch traversal reaches a kernel.
/// ´claim:matrix:owner-sigmoid-observed-reuse´
/// ´test:integration:owner-sigmoid-observed-reuse´
#[test]
fn owner_sigmoid_observed_reuse() -> TestResult {
    use nrz_matrix::{PreparedSigmoid, SigmoidPreparationPolicy};
    use nrz_scalar::SigmoidInput;
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    let observer = Observer::default();
    tracing::subscriber::with_default(observer.clone(), || -> TestResult {
        let mut f = Fixture::new()?;
        let r = sigmoid_request(&mut f)?;
        let context = ArithmeticContext::new()?;
        let mut budget = ledger()?;
        let mut counts = Counts::default();
        reject_coordinates(&mut f, &r, &observer, &context)?;
        let owner = PreparedSigmoid::new(
            &mut f.cas,
            &r,
            [0.0, 2.0],
            40,
            SigmoidPreparationPolicy {
                bits: 40,
                requested_bits: 44,
                revision: 1,
            },
            Policy::default(),
            &context,
            &mut budget,
            &mut counts,
        )?;
        let admitted_fields = *observer.formatted.lock().map_err(|_| "format counters")?;
        let events = observer.exact_events.load(Ordering::Relaxed);
        let before = budget.usage();
        for x in [1.0, 0.0, 2.0] {
            owner.evaluate(
                SigmoidInput {
                    endpoints: [x, x],
                    center: x,
                    bits: 40,
                },
                &context,
            )?;
        }
        assert_eq!(
            observer.exact_events.load(Ordering::Relaxed),
            events,
            "no exact lifecycle in native reuse"
        );
        assert_eq!(budget.usage(), before);
        assert_eq!(
            observer.formatted.lock().map_err(|_| "format counters")?[0] - admitted_fields[0],
            3,
            "each admitted input is formatted once"
        );
        assert_eq!(
            observer.formatted.lock().map_err(|_| "format counters")?[2],
            admitted_fields[2],
            "native spans must not format recursive owner authority"
        );
        for limits in [
            Limits {
                matrix_entries: 0,
                ..Limits::default()
            },
            Limits {
                work: 0,
                ..Limits::default()
            },
        ] {
            let mut refused = Budget::new(limits)?;
            let before_fields = *observer.formatted.lock().map_err(|_| "format counters")?;
            let kernels = observer.kernels.load(Ordering::Relaxed);
            let allocations = observer.constructions.load(Ordering::Relaxed);
            owner
                .evaluate_batch(
                    &[SigmoidInput {
                        endpoints: [1.0, 1.0],
                        center: 1.0,
                        bits: 40,
                    }; 3],
                    &context,
                    &mut refused,
                )
                .expect_err("preflight refusal");
            assert_eq!(observer.kernels.load(Ordering::Relaxed), kernels);
            assert_eq!(
                observer.constructions.load(Ordering::Relaxed),
                allocations,
                "no allocation before admission"
            );
            assert_eq!(
                *observer.formatted.lock().map_err(|_| "format counters")?,
                before_fields,
                "count and Work preflight must not enumerate supplied records"
            );
        }
        assert!(
            observer.formatted.lock().map_err(|_| "format counters")?[0] > 0,
            "admitted operands are actually formatted"
        );
        Ok(())
    })
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

/// Visit Debug fields as a formatting subscriber does, without retaining events.
struct FormattingVisitor(Arc<Mutex<[usize; 4]>>);
impl tracing::field::Visit for FormattingVisitor {
    fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        let mut text = String::new();
        write!(&mut text, "{value:?}").expect("String formatting");
        // Preserve the original test failure if ledger-drop events arrive during unwinding.
        let mut counts = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (count, name) in
            counts
                .iter_mut()
                .zip(["SigmoidInput {", "CoordinateBound {", "PreparedSigmoid {"])
        {
            *count += text.matches(name).count();
        }
        counts[0] += text.matches("TanhInput {").count();
        counts[3] += text.matches("TanhOutput {").count();
        counts[2] += text.matches("TanhProgram {").count()
            + text.matches("SigmoidProgram {").count()
            + text.matches("Expm1Program {").count()
            + text.matches("Budget {").count();
    }
}

/// Build the admitted original sigmoid box without any observer-specific shortcut.
fn sigmoid_request(
    f: &mut Fixture,
) -> Result<nrz_bindings::readings::algebra::enclosure::EnclosureRequest, Box<dyn std::error::Error>>
{
    use nrz_bindings::readings::algebra::{
        Rational, Reference,
        enclosure::Interval,
        syntax::{Draft, ExpressionDraft, Operator},
    };
    let mut r = broad(f, 40)?.original;
    let argument = r.coordinates[0].coordinate.clone();
    let header = f.cas.expression_header(&argument)?;
    let sort = f.cas.expression(&argument)?.1.clone();
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        operator: Operator::Sigmoid,
        sort,
        header: Reference::Existing(header),
        scope: Vec::new(),
        binding: None,
        operands: vec![Reference::Existing(argument)],
    });
    r.target = f
        .cas
        .admit(&[draft], &mut ledger()?)?
        .into_iter()
        .next()
        .ok_or("sigmoid")?;
    r.coordinates[0].interval = Interval::new(
        Rational::zero(),
        Rational::from_ratio(2, 1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    Ok(r)
}

/// Excess coordinates are refused without formatting their records.
fn reject_coordinates(
    f: &mut Fixture,
    r: &nrz_bindings::readings::algebra::enclosure::EnclosureRequest,
    observer: &Observer,
    context: &ArithmeticContext,
) -> TestResult {
    use nrz_matrix::{PreparedSigmoid, SigmoidPreparationPolicy};
    let mut budget = ledger()?;
    let mut counts = Counts::default();
    let mut excessive = r.clone();
    excessive.coordinates.push(excessive.coordinates[0].clone());
    let before_fields = *observer.formatted.lock().map_err(|_| "format counters")?;
    let rejected = PreparedSigmoid::new(
        &mut f.cas,
        &excessive,
        [0.0, 2.0],
        40,
        SigmoidPreparationPolicy {
            bits: 40,
            requested_bits: 44,
            revision: 1,
        },
        Policy::default(),
        context,
        &mut budget,
        &mut counts,
    );
    assert!(matches!(
        rejected,
        Err(nrz_matrix::PreparedSigmoidError::Domain)
    ));
    assert_eq!(
        *observer.formatted.lock().map_err(|_| "format counters")?,
        before_fields,
        "coordinate admission must precede formatting supplied records"
    );
    Ok(())
}

/// Scalar tanh formats each public input once without recursive basis or owner fields and emits no exact Work event.
/// # Panics
/// Panics if field visitation walks the basis, duplicates an input or reaches exact accounting.
/// ´claim:matrix:scalar-tanh-observation´
/// ´test:integration:scalar-tanh-observation´
#[test]
fn scalar_tanh_observation() -> TestResult {
    use nrz_scalar::{Expm1Parameters, Expm1Program, TanhInput, TanhProgram};
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    let context = ArithmeticContext::new()?;
    let basis = Expm1Program::new(
        &[1.0],
        Expm1Parameters {
            domain: [-2.0, 0.0],
            bits: 44,
            radius_bits: 44,
            absolute_error: 2.0_f64.powi(-44),
            environment: 1,
        },
        &context,
    )?;
    let observer = Observer::default();
    tracing::subscriber::with_default(observer.clone(), || -> TestResult {
        let program = TanhProgram::new(&basis, [0.0, 1.0], 40)?;
        for x in [0.0, 2.0_f64.powi(-60), f64::from_bits(1)] {
            let _result = program.evaluate(
                TanhInput {
                    endpoints: [x, x],
                    center: x,
                    bits: 40,
                },
                &context,
            );
        }
        let fields = *observer.formatted.lock().map_err(|_| "field counters")?;
        assert_eq!(fields[0], 3, "one public input per scalar invocation");
        assert_eq!(
            fields[3], 2,
            "one complete output per successful invocation"
        );
        assert_eq!(fields[2], 0, "no recursive authority or program formatting");
        assert_eq!(
            observer.exact_events.load(Ordering::Relaxed),
            0,
            "native arithmetic has no exact Work"
        );
        Ok(())
    })
}
