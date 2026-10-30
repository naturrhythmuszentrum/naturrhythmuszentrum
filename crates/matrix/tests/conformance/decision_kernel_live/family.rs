// SPDX-License-Identifier: AGPL-3.0-only
//! Physical duration families retain their explicit assumptions through a complete live decision.

mod admission;
mod costs;
mod mapped;

use super::{TestResult, inputs, ledger, pi, select};
use crate::conformance::decision_fixture::{SelectionFixture, family_fixture};
use nrz_bindings::readings::algebra::{
    Cas, ExactConstant, Location, Rational,
    decision::{EnclosureOrigin, ScalarKernelLaw},
    proof::{Certificate, Statement},
    syntax::Constant,
};
use nrz_bindings::{
    BaseDimension, Budget, Context, CountDomain, DecisionExpression, DecisionInputs, Declaration,
    Descriptor, Dimension, Evidence, ExponentPolicy, IntegerDecisionCase, IntegerDecisionSpec,
    Kind, Measure, Observable, Registry, Role, Scale,
};
use nrz_matrix::{DecisionSnapshotError, Heap, HeapError, HeapLimits, LiveDecisionError};
use nrz_scalar::ArithmeticContext;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Physically checked open duration and a declared exact LC period share one immutable owner.
fn physical() -> TestResult<Rc<nrz_bindings::CheckedDeclarations>> {
    let time = Kind::Named(
        "duration".into(),
        Box::new(Dimension::base(BaseDimension::Time)),
    );
    let duration = Descriptor::new(time.clone(), Measure::coherent(), Role::Difference)?;
    let scale = ExactConstant::named(Constant::Pi, &mut ledger()?)?.div(
        &Rational::from_ratio(1000, 1, &mut ledger()?)?.into(),
        &mut ledger()?,
    )?;
    let period = Descriptor::new(
        time,
        Measure::new("full LC period", Scale::Expression(scale), Rational::zero())?,
        Role::Difference,
    )?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["duration", "period", "quotient"],
    )?;
    owner.declare(
        "duration".into(),
        Declaration::Input {
            descriptor: duration,
            nonzero: true,
            evidence: Evidence {
                source: "measured-duration-family".into(),
                assurance: "explicit deterministic box".into(),
            },
        },
    )?;
    owner.declare(
        "period".into(),
        Declaration::Constant {
            descriptor: period,
            value: Rational::one(),
            evidence: Evidence::default(),
        },
    )?;
    let expression = owner.parse("physical LC family quotient", "duration/period")?;
    owner.declare(
        "quotient".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence::default(),
        }),
    )?;
    Ok(Rc::new(owner.freeze()?))
}

/// Both physical declaration correspondence and the exact box premises precede heap import.
fn source(budget: &mut Budget) -> TestResult<(DecisionInputs, SelectionFixture)> {
    let (source, family) = family_fixture()?;
    let physical = DecisionExpression::family(
        &source.frame.cas,
        physical()?,
        "quotient",
        &source.context,
        vec![("duration".into(), family)],
        budget,
    )?;
    assert!(physical.exact().is_none());
    let association = associate(&source, physical, budget)?;
    Ok((association, source))
}

/// Native and mapped physical declarations share the complete case and association checks.
fn associate(
    source: &SelectionFixture,
    physical: DecisionExpression,
    budget: &mut Budget,
) -> TestResult<DecisionInputs> {
    let proposal = source.steps.last().ok_or("selection")?;
    let Statement::IntegerSelection(selection) = &proposal.conclusion else {
        return Err("selection statement".into());
    };
    let Certificate::IntegerSelection(enclosure) = &proposal.certificate else {
        return Err("selection certificate".into());
    };
    let case = IntegerDecisionCase::declare_family(
        &source.frame.cas,
        physical,
        IntegerDecisionSpec {
            context: source.context.clone(),
            family: selection.family.clone(),
            target: enclosure.target.clone(),
            parameters: Vec::new(),
            count: Descriptor::new(
                Kind::Count("complete_lc_cycles".into(), CountDomain::NONNEGATIVE),
                Measure::coherent(),
                Role::Difference,
            )?,
        },
        budget,
    )?;
    Ok(case.associate_inputs(budget)?)
}

/// The full family selects three cycles, survives fresh replay and retires on a radius-only write.
/// ´test:integration:kernel-physical-family-lifecycle´
/// ´claim:matrix:kernel-physical-family-lifecycle´
#[test]
fn kernel_physical_family_lifecycle() -> TestResult {
    let mut setup = ledger()?;
    let (association, mut source) = source(&mut setup)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let heap_before = heap.provenance_usage();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior live family"))?;
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut budget)?;
    let captured = budget.usage().work;
    let trace = heap.trace_ratio_decision(
        frozen,
        &mut source.frame.cas,
        &pi()?,
        &ArithmeticContext::new()?,
        &mut budget,
    )?;
    let traced = budget.usage().work;
    let retained = trace.retained_payload_bytes();
    let EnclosureOrigin::ScalarKernel(certificate) = &trace.evidence().contract().origin else {
        return Err("scalar family origin".into());
    };
    assert_eq!(certificate.version, 3);
    assert!(matches!(
        certificate.records[0].law,
        ScalarKernelLaw::Family(_)
    ));
    assert_eq!(trace.evidence().scalar_usage().work, 732);
    let input = &trace.inputs().operands()[0];
    let handle = input.input().handle();
    let stamp = input.input().stamp();
    let center = input.coordinates()[0];
    let witness = select(&mut source, &trace, &mut budget)?;
    let selected = budget.usage().work;
    assert_eq!(witness.hypotheses().len(), 3);
    let mut wire = ledger()?;
    let bytes = source.frame.cas.export_witness(&witness, &mut wire)?;
    let exported = wire.usage().work;
    let mut receiver = Cas::new(nrz_bindings::Limits::default())?;
    let fresh = receiver.inspect(&bytes, &mut wire)?;
    let replayed = wire.usage().work - exported;
    assert_eq!(fresh.hypotheses().len(), 3);
    assert_eq!(receiver.export_witness(&fresh, &mut ledger()?)?, bytes);
    let prepared = heap.prepare_ratio_kernel_decision(
        trace,
        Arc::new(Mutex::new(source.frame.cas)),
        witness,
        &mut budget,
    )?;
    let prepared_work = budget.usage().work;
    let published = heap.publish_decision(prepared, &mut budget)?;
    let publication = budget.usage().work;
    let count = published.count(&heap, 3, &mut budget)?;
    assert_eq!(count.current(&heap, &mut budget)?.value(), 3);
    let current = budget.usage().work;
    let layout = heap.value(handle)?.region().layout().clone();
    let alias = heap.alias(handle, &layout)?;
    heap.replace_declared_bounds(alias, stamp.version(), &[center], &[1.0 / 1024.0])?;
    let Err(fault) = count.current(&heap, &mut budget) else {
        panic!("changed family radius must retire the attachment");
    };
    assert!(matches!(
        fault,
        LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
    ));
    let Err(fault) = published.current(&heap, &mut budget) else {
        panic!("retired family attachment must refuse");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    assert_eq!(heap.provenance_usage(), heap_before);
    assert_eq!(setup.usage().work, 96_200);
    assert_eq!(captured - 17, 2952);
    assert_eq!(traced - captured, 312_744);
    assert_eq!(selected - traced, 328_319);
    assert_eq!(prepared_work - selected, 5659);
    assert_eq!(publication - prepared_work, 193);
    assert_eq!(current - publication, 191);
    assert_eq!(budget.usage().work - current, 35);
    assert_eq!(budget.usage().work, 650_110);
    assert_eq!(setup.usage().work + budget.usage().work - 17, 746_293);
    assert_eq!(retained, 8672);
    assert_eq!(exported, 2487);
    assert_eq!(bytes.len(), 2487);
    assert_eq!(replayed, 485_064);
    Ok(())
}
