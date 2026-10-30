// SPDX-License-Identifier: AGPL-3.0-only
//! Complete question identities and all original dependencies gate every live tuple consumer.

use super::{decision_fixture::ledger, lattice_live, lattice_selection};
use crate::binding_testing::TestResult;
use nrz_bindings::{
    DecisionError, LatticeInputs,
    readings::algebra::{Rational, lattice::RationalBoxEnclosure},
};
use nrz_matrix::{
    Coverage, DecisionSnapshotError, Heap, HeapError, HeapLimits, LiveDecisionError,
    PublishedLatticeDecision, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

/// One selected torus and its complete original native inventory.
struct Fixture {
    /// Current native storage owner.
    heap: Heap,
    /// Immutable complete physical question and designated symbols.
    association: LatticeInputs,
    /// Current selected native bindings in original association order.
    inputs: Vec<ValueBinding<f64>>,
    /// Complete selected tuple and shared retirement authority.
    published: PublishedLatticeDecision<f64>,
    /// Independently certified original target box.
    contract: RationalBoxEnclosure,
}

/// Build a positive declared torus before mutating one named owner.
fn fixture() -> TestResult<Fixture> {
    let source = lattice_selection::prepared(1, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let contract = lattice_selection::original_box(&source)?.clone();
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = lattice_live::inputs(&mut heap, &association, 1)?;
    let (prepared, _) =
        lattice_live::candidate(&heap, source, &association, &inputs, &mut ledger()?)?;
    let published = heap.publish_lattice_decision(prepared, &mut ledger()?)?;
    Ok(Fixture {
        heap,
        association,
        inputs,
        published,
        contract,
    })
}

/// Target, full-turn and offset sources all retire the whole tuple and every descendant.
/// ´test:integration:lattice-live-complete-dependency-retirement´
/// ´claim:matrix:lattice-live-complete-dependency-retirement´
#[test]
fn lattice_live_complete_dependency_retirement() -> TestResult {
    for slot in 0..5 {
        let mut fixture = fixture()?;
        let frozen = fixture.heap.freeze_lattice_inputs(
            &fixture.association,
            &fixture.inputs,
            &mut ledger()?,
        )?;
        let operand = &frozen.operands()[slot];
        let handle = operand.input().handle();
        let alias = fixture.heap.alias(handle, operand.input().layout())?;
        let center = operand.coordinates()[0];
        let representative = fixture
            .published
            .representative(&fixture.heap, 1, &mut ledger()?)?;
        let first = fixture
            .published
            .index(&fixture.heap, 0, (0, 3), &mut ledger()?)?;
        let second = fixture
            .published
            .index(&fixture.heap, 1, (0, 3), &mut ledger()?)?;
        let archived = fixture
            .published
            .current(&fixture.heap, &mut ledger()?)?
            .clone();
        let graph = archived.cas().counts();
        let records = fixture.heap.provenance_usage();
        fixture.heap.replace_declared_bounds(
            alias,
            operand.input().stamp().version(),
            &[center],
            &[1.5],
        )?;
        assert_eq!(
            fixture.heap.value(handle)?.region().get(0, 0)?.to_bits(),
            center.to_bits()
        );
        let Err(fault) = representative.current(&fixture.heap, &mut ledger()?) else {
            panic!("any changed original must retire the complete representative");
        };
        assert!(matches!(
            fault,
            LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
        ));
        for index in [&first, &second] {
            let Err(fault) = index.current(&fixture.heap, &mut ledger()?) else {
                panic!("every coordinate lowering must follow the shared retirement");
            };
            assert!(matches!(fault, LiveDecisionError::Retired));
        }
        let Err(fault) = fixture.published.current(&fixture.heap, &mut ledger()?) else {
            panic!("the parent must remain retired");
        };
        assert!(matches!(fault, LiveDecisionError::Retired));
        assert_eq!(archived.indices().len(), 2);
        assert_eq!(archived.cas().counts(), graph);
        assert_eq!(fixture.heap.provenance_usage(), records);
    }
    Ok(())
}

/// Equal independent declarations cannot authenticate an existing physical publication.
/// ´test:integration:lattice-live-question-identity´
/// ´claim:matrix:lattice-live-question-identity´
#[test]
fn lattice_live_question_identity() -> TestResult {
    let fixture = fixture()?;
    let independent = lattice_selection::prepared(1, false)?
        .case
        .associate_inputs(&mut ledger()?)?;
    let Err(LiveDecisionError::Admission(fault)) =
        fixture
            .published
            .current_for(&fixture.heap, &independent, &mut ledger()?)
    else {
        panic!("independent equal declarations must refuse at attachment authentication");
    };
    assert!(matches!(*fault, DecisionError::Attachment));
    fixture
        .published
        .current_for(&fixture.heap, &fixture.association, &mut ledger()?)?;
    Ok(())
}

/// Invalidate the old native lease, then bind the widened assumption to its declared symbols.
fn widen(fixture: &mut Fixture) -> TestResult {
    let frozen = fixture.heap.freeze_lattice_inputs(
        &fixture.association,
        &fixture.inputs,
        &mut ledger()?,
    )?;
    let original = &frozen.operands()[0];
    let center = original.coordinates()[0];
    fixture.heap.replace_declared_bounds(
        original.input().handle(),
        original.input().stamp().version(),
        &[center],
        &[1.5],
    )?;
    let Err(fault) = fixture.published.current(&fixture.heap, &mut ledger()?) else {
        panic!("radius-only widening must retire the old tuple");
    };
    assert!(matches!(
        fault,
        LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
    ));
    fixture.inputs[0] = fixture.heap.import_symbols(
        fixture.association.operands()[0].symbols(),
        &[center],
        Some(&[1.5]),
        Some(&[0.0]),
        Coverage::DeclaredBounds,
    )?;
    Ok(())
}

/// A widened live box cannot borrow the narrow theorem, even after its own original replay.
/// ´test:integration:lattice-live-widening-refuses-old-selection´
/// ´claim:matrix:lattice-live-widening-refuses-old-selection´
#[test]
fn lattice_live_widening_refuses_old_selection() -> TestResult {
    let mut fixture = fixture()?;
    let selected = fixture
        .published
        .current(&fixture.heap, &mut ledger()?)?
        .clone();
    let graph = selected.cas().counts();
    widen(&mut fixture)?;
    let records = fixture.heap.provenance_usage();
    let frozen = fixture.heap.freeze_lattice_inputs(
        &fixture.association,
        &fixture.inputs,
        &mut ledger()?,
    )?;
    let Err(fault) = fixture.heap.enclose_lattice_inputs(
        frozen,
        selected.cas(),
        &fixture.contract,
        &ArithmeticContext::new()?,
        lattice_live::literals(),
        &mut ledger()?,
    ) else {
        panic!("the widened native box must refuse narrow original containment");
    };
    assert!(matches!(fault, LiveDecisionError::Enclosure));
    fixture.contract.coordinates[0].lower = Rational::parse("1799/100", &mut ledger()?)?;
    fixture.contract.coordinates[0].upper = Rational::parse("2101/100", &mut ledger()?)?;
    let frozen = fixture.heap.freeze_lattice_inputs(
        &fixture.association,
        &fixture.inputs,
        &mut ledger()?,
    )?;
    let enclosed = fixture.heap.enclose_lattice_inputs(
        frozen,
        selected.cas(),
        &fixture.contract,
        &ArithmeticContext::new()?,
        lattice_live::literals(),
        &mut ledger()?,
    )?;
    let Err(fault) =
        fixture
            .heap
            .prepare_lattice_decision(enclosed, selected.clone(), &mut ledger()?)
    else {
        panic!("a wider certified box must not reuse the old selection box");
    };
    assert!(matches!(fault, LiveDecisionError::Enclosure));
    assert_eq!(selected.cas().counts(), graph);
    assert_eq!(fixture.heap.provenance_usage(), records);
    Ok(())
}

/// Foreign, resource and executable-capacity refusals never retire an otherwise valid tuple.
/// ´test:integration:lattice-live-transient-refusals´
/// ´claim:matrix:lattice-live-transient-refusals´
#[test]
fn lattice_live_transient_refusals() -> TestResult {
    use nrz_bindings::{
        Budget, Limits,
        readings::algebra::{FaultKind, InvalidCode, Location},
    };
    let fixture = fixture()?;
    let foreign = Heap::new(HeapLimits::default())?;
    let Err(fault) = fixture.published.current(&foreign, &mut ledger()?) else {
        panic!("a foreign heap must refuse the current lease");
    };
    assert!(matches!(
        fault,
        LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::ForeignHandle))
    ));
    let mut budget = Budget::new(Limits {
        work: 17,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior live lattice caller"))?;
    let Err(LiveDecisionError::Algebra(fault)) =
        fixture.published.current(&fixture.heap, &mut budget)
    else {
        panic!("exhausted current-access Work must refuse");
    };
    assert!(matches!(fault.kind, FaultKind::Exhausted { .. }));
    assert_eq!(budget.usage().work, 17);
    let Err(LiveDecisionError::Algebra(fault)) =
        fixture
            .published
            .index(&fixture.heap, 0, (0, 2), &mut ledger()?)
    else {
        panic!("capacity below the selected first Integer must refuse");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
    );
    let selected = fixture.published.current(&fixture.heap, &mut ledger()?)?;
    assert_eq!(
        selected.indices(),
        [Rational::from_ratio(3, 1, &mut ledger()?)?, Rational::one()]
    );
    assert_eq!(
        fixture
            .published
            .index(&fixture.heap, 1, (0, 1), &mut ledger()?)?
            .current(&fixture.heap, &mut ledger()?)?,
        1
    );
    Ok(())
}
