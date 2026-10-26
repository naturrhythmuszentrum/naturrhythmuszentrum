// SPDX-License-Identifier: AGPL-3.0-only
//! Complete physical associations are matched to the actual versioned LC inputs.

use super::decision_fixture::{fixture, ledger};
use crate::binding_testing::TestResult;
use nrz_bindings::{
    BaseDimension, Context, CountDomain, DecisionExpression, DecisionInputs, Declaration,
    Descriptor, Dimension, Evidence, ExponentPolicy, IntegerDecisionCase, IntegerDecisionSpec,
    Kind, Measure, Observable, Registry, Role, Scale,
    readings::algebra::{
        ExactConstant, Rational,
        proof::{Certificate, Statement},
        syntax::Constant,
    },
};
use nrz_matrix::{Coverage, DecisionSnapshotError, Heap, HeapError, HeapLimits, ValueBinding};
use std::rc::Rc;

/// Exact duration and original full-period map, retained by one physical decision owner.
fn association() -> TestResult<DecisionInputs> {
    let source = fixture(false, "3", "10/Pi")?;
    let mut budget = ledger()?;
    let time = Kind::Named(
        "duration".into(),
        Box::new(Dimension::base(BaseDimension::Time)),
    );
    let second = Descriptor::new(time.clone(), Measure::coherent(), Role::Difference)?;
    let pi = ExactConstant::named(Constant::Pi, &mut budget)?;
    let divisor = Rational::from_ratio(1000, 1, &mut budget)?;
    let period = Descriptor::new(
        time,
        Measure::new(
            "period coordinate",
            Scale::Expression(pi.div(&divisor.into(), &mut budget)?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["duration", "period", "quotient"],
    )?;
    for (name, descriptor, value) in [
        (
            "duration",
            second,
            Rational::from_ratio(1, 100, &mut budget)?,
        ),
        ("period", period, Rational::one()),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor,
                value,
                evidence: Evidence::default(),
            },
        )?;
    }
    let expression = owner.parse("original LC ratio", "duration/period")?;
    owner.declare(
        "quotient".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence::default(),
        }),
    )?;
    let physical = DecisionExpression::declared(Rc::new(owner.freeze()?), "quotient", &mut budget)?;
    let proposal = source.steps.last().ok_or("missing selection")?;
    let Statement::IntegerSelection(selection) = &proposal.conclusion else {
        return Err("missing Integer selection".into());
    };
    let Certificate::IntegerSelection(enclosure) = &proposal.certificate else {
        return Err("missing enclosure".into());
    };
    let case = IntegerDecisionCase::declare(
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
        &mut budget,
    )?;
    Ok(case.associate_inputs(&mut budget)?)
}

/// Import against the designated symbols; equal independently declared inputs cannot substitute.
fn inputs(heap: &mut Heap, association: &DecisionInputs) -> TestResult<Vec<ValueBinding<f64>>> {
    association
        .operands()
        .iter()
        .map(|operand| {
            let center = match operand.name() {
                "duration" => 1311.0_f64 / 131_072.0,
                "period" => 103.0_f64 / 32_768.0,
                _ => return Err("unexpected original input".into()),
            };
            Ok(heap.import_symbols(
                operand.symbols(),
                &[center],
                Some(&[1.0 / 262_144.0]),
                Some(&[0.0]),
                Coverage::DeclaredBounds,
            )?)
        })
        .collect()
}

/// Probe the standalone snapshot owner, separately from the preexisting copy lease.
/// ´test:integration:decision-frozen-inputs-costs´
#[test]
fn decision_frozen_inputs_costs() -> TestResult {
    let association = association()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association)?;
    let before = heap.provenance_usage();
    let mut budget = ledger()?;
    let frozen = heap.freeze_decision_inputs(&association, &inputs, &mut budget)?;
    let capture = budget.usage().work;
    heap.check_decision_inputs(&frozen, &mut budget)?;
    assert_eq!(frozen.operands().len(), 2);
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(capture, 2952);
    assert_eq!(budget.usage().work - capture, 65);
    assert_eq!(frozen.retained_payload_bytes(), 2880);
    Ok(())
}

/// Foreign heaps and radius-only alias writes invalidate the complete frozen input authority.
/// ´test:integration:decision-frozen-inputs-lifetime´
#[test]
fn decision_frozen_inputs_lifetime() -> TestResult {
    let association = association()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
    let foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        foreign.check_decision_inputs(&frozen, &mut ledger()?),
        Err(DecisionSnapshotError::Heap(HeapError::ForeignHandle))
    ));
    let original = frozen.operands()[0].input();
    let handle = original.handle();
    let stamp = original.stamp();
    let alias = heap.alias(handle, original.layout())?;
    let coordinates = frozen.operands()[0].coordinates();
    let before = heap.provenance_usage();
    heap.replace_declared_bounds(alias, stamp.version(), &[coordinates[0]], &[1.0 / 1024.0])?;
    assert_eq!(
        heap.value(handle)?.region().get(0, 0)?.to_bits(),
        coordinates[0].to_bits()
    );
    assert!(matches!(
        heap.check_decision_inputs(&frozen, &mut ledger()?),
        Err(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
    ));
    let stale = heap
        .freeze_decision_inputs(&association, &bindings, &mut ledger()?)
        .is_err();
    assert!(stale);
    assert_eq!(
        frozen.operands()[0].coordinates()[1].to_bits(),
        (1.0_f64 / 262_144.0).to_bits()
    );
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}

/// Snapshot Work and both payload ceilings have literal success and one-below boundaries.
/// ´test:integration:decision-frozen-inputs-boundaries´
#[test]
fn decision_frozen_inputs_boundaries() -> TestResult {
    let association = association()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association)?;
    let before = heap.provenance_usage();
    for (work, staged, output, accepted) in [
        (2969, 2909, 2880, true),
        (2968, 2909, 2880, false),
        (2969, 2908, 2880, false),
        (2969, 2909, 2879, false),
    ] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            staged_payload_bytes: staged,
            output_bytes: output,
            ..nrz_bindings::Limits::default()
        })?;
        let at = nrz_bindings::readings::algebra::Location::new("prior snapshot caller");
        budget.work(17, at)?;
        let result = budget.with_retained_payload(13, at, |budget| {
            heap.freeze_decision_inputs(&association, &inputs, budget)
        });
        if accepted {
            assert_eq!(result?.operands().len(), 2);
            assert_eq!(budget.usage().work, 2969);
        } else {
            assert!(matches!(result, Err(DecisionSnapshotError::Resources(_))));
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        assert_eq!(heap.provenance_usage(), before);
    }
    let frozen = heap.freeze_decision_inputs(&association, &inputs, &mut ledger()?)?;
    for (work, accepted) in [(82, true), (81, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior recheck caller"),
        )?;
        let result = heap.check_decision_inputs(&frozen, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, 82);
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
    }
    heap.check_decision_inputs(&frozen, &mut ledger()?)?;
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}
