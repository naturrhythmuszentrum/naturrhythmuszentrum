// SPDX-License-Identifier: AGPL-3.0-only
//! Live widened evidence reaches typed CAS outcomes without reviving archived descendants.

use super::{decision_fixture::ledger, lattice_live, lattice_revalidation, lattice_selection};
use crate::binding_testing::TestResult;
use nrz_bindings::{
    Budget, LatticeInputs, SelectedLattice,
    readings::algebra::{
        Rational,
        lattice::{
            LatticeEnclosure, LatticeSelection, LatticeSelectionOutcome, LatticeUndecidedReason,
        },
        proof::VerifiedWitness,
    },
};
use nrz_matrix::{
    Coverage, FrozenLatticeEnclosure, Heap, HeapLimits, LiveDecisionError, LiveLatticeIndex,
    LiveLatticeRepresentative, PublishedLatticeDecision, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

/// One retired authority and fresh native bindings for its identical declared question.
pub(super) struct Fixture {
    /// Current native source owner.
    pub(super) heap: Heap,
    /// Complete declared physical sources.
    association: LatticeInputs,
    /// Fresh versions after the old source lease was invalidated.
    inputs: Vec<ValueBinding<f64>>,
    /// Permanently retired native tuple.
    published: PublishedLatticeDecision<f64>,
    /// Immutable mathematical tuple and guarded graph, independent of its former live lease.
    pub(super) archived: SelectedLattice,
    /// Dependent executable coordinate of the retired publication.
    index: LiveLatticeIndex<f64>,
    /// Dependent physical coordinate of the retired publication.
    representative: LiveLatticeRepresentative<f64>,
    /// Original finite cover theorem retained for a separately prepared attempt.
    pub(super) cover: VerifiedWitness,
    /// Same complete named candidate, with no authority for the changed live box.
    pub(super) expected: LatticeSelection,
    /// Original and affine enclosures for the new native box.
    pub(super) evidence: LatticeEnclosure,
}

/// Change only the first radius, retire all old authority and capture replacement source versions.
pub(super) fn fixture(kind: u8, wide: bool) -> TestResult<Fixture> {
    let source = lattice_selection::prepared(kind, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let cover = source.cover.clone();
    let expected = source.expected.clone();
    let mut evidence = source.enclosure.clone();
    if wide {
        lattice_revalidation::widen(&mut evidence)?;
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = lattice_live::inputs(&mut heap, &association, kind)?;
    let (prepared, handle) =
        lattice_live::candidate(&heap, source, &association, &inputs, &mut ledger()?)?;
    let published = heap.publish_lattice_decision(prepared, &mut ledger()?)?;
    let archived = published.current(&heap, &mut ledger()?)?.clone();
    let index = published.index(&heap, 0, (0, 3), &mut ledger()?)?;
    let representative = published.representative(&heap, 0, &mut ledger()?)?;
    let value = heap.value(handle)?;
    let version = value.stamp().version();
    let center = value.region().get(0, 0)?;
    let radius = if wide {
        1.5
    } else if kind == 0 {
        129.0 / 1024.0
    } else {
        513.0 / 1024.0
    };
    heap.replace_declared_bounds(handle, version, &[center], &[radius])?;
    let Err(fault) = published.current(&heap, &mut ledger()?) else {
        panic!("a changed radius must retire the old publication");
    };
    assert!(matches!(fault, LiveDecisionError::Snapshot(_)));
    inputs[0] = heap.import_symbols(
        association.operands()[0].symbols(),
        &[center],
        Some(&[radius]),
        Some(&[0.0]),
        Coverage::DeclaredBounds,
    )?;
    Ok(Fixture {
        heap,
        association,
        inputs,
        published,
        archived,
        index,
        representative,
        cover,
        expected,
        evidence,
    })
}

/// Fresh capture and containment use the same original physical question and guarded store.
pub(super) fn enclose(
    fixture: &Fixture,
    budget: &mut Budget,
) -> TestResult<FrozenLatticeEnclosure<f64>> {
    let frozen =
        fixture
            .heap
            .freeze_lattice_inputs(&fixture.association, &fixture.inputs, budget)?;
    let LatticeEnclosure::Original(original) = &fixture.evidence else {
        return Err("original box required".into());
    };
    Ok(fixture.heap.enclose_lattice_inputs(
        frozen,
        &mut *fixture.archived.cas()?,
        &original.original,
        &ArithmeticContext::new()?,
        lattice_live::literals(),
        budget,
    )?)
}

/// Every dependent retains the old monotone authority after either revalidation outcome.
pub(super) fn retired(fixture: &Fixture) -> TestResult {
    let Err(fault) = fixture.published.current(&fixture.heap, &mut ledger()?) else {
        panic!("the old tuple must remain retired");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    let Err(fault) = fixture.index.current(&fixture.heap, &mut ledger()?) else {
        panic!("the old executable coordinate must remain retired");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    let Err(fault) = fixture
        .representative
        .current(&fixture.heap, &mut ledger()?)
    else {
        panic!("the old physical coordinate must remain retired");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    Ok(())
}

/// The widened live torus returns complete typed boundary evidence and preserves the archived graph.
/// ´test:integration:lattice-live-widened-undecided´
/// ´claim:matrix:lattice-live-widened-undecided´
#[test]
fn lattice_live_widened_undecided() -> TestResult {
    let fixture = fixture(1, true)?;
    let bytes = fixture.archived.cas()?.canonical_snapshot(&mut ledger()?)?;
    let records = fixture.heap.provenance_usage();
    let mut budget = ledger()?;
    let enclosed = enclose(&fixture, &mut budget)?;
    let containment = budget.usage().work;
    let mut cas = fixture.archived.cas()?;
    let attempt = cas.prepare_lattice_selection(
        &fixture.cover,
        &fixture.expected,
        &fixture.evidence,
        &mut budget,
    )?;
    let preparation = budget.usage().work;
    let LatticeSelectionOutcome::Undecided(outcome) =
        fixture
            .heap
            .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut budget)?
    else {
        panic!("the widened live box must return typed undecided");
    };
    assert_eq!(outcome.reason, LatticeUndecidedReason::BoundaryIntersection);
    assert!(std::ptr::eq(outcome.evidence, &raw const attempt));
    assert_eq!(outcome.evidence.candidate(), &fixture.expected);
    assert_eq!(outcome.evidence.enclosure(), &fixture.evidence);
    assert_eq!(outcome.spent_work, budget.usage().work);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, bytes);
    drop(cas);
    retired(&fixture)?;
    assert_eq!(fixture.heap.provenance_usage(), records);
    assert_eq!(containment, 27_635);
    assert_eq!(preparation, 156_853);
    assert_eq!(budget.usage().work, 328_414);
    Ok(())
}

/// Same-cell phase and torus twins retain canonical witnesses and publish only after a fresh lease.
/// ´test:integration:lattice-live-revalidation-narrow-twins´
/// ´claim:matrix:lattice-live-revalidation-narrow-twins´
#[test]
fn lattice_live_revalidation_narrow_twins() -> TestResult {
    for kind in [0, 1] {
        let fixture = fixture(kind, false)?;
        let enclosed = enclose(&fixture, &mut ledger()?)?;
        let mut cas = fixture.archived.cas()?;
        let original = cas.export_witness(fixture.archived.witness(), &mut ledger()?)?;
        let attempt = cas.prepare_lattice_selection(
            &fixture.cover,
            &fixture.expected,
            &fixture.evidence,
            &mut ledger()?,
        )?;
        let LatticeSelectionOutcome::Decided(witness) =
            fixture
                .heap
                .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut ledger()?)?
        else {
            panic!("a same-cell revalidation must select the complete tuple");
        };
        assert_eq!(cas.export_witness(&witness, &mut ledger()?)?, original);
        let outputs = cas.lattice_selection_outputs(&witness, &mut ledger()?)?;
        drop(cas);
        let selected = fixture.archived.admit_outputs(&outputs, &mut ledger()?)?;
        let prepared = fixture
            .heap
            .prepare_lattice_decision(enclosed, selected, &mut ledger()?)?;
        let replacement = fixture
            .heap
            .publish_lattice_decision(prepared, &mut ledger()?)?;
        let expected = if kind == 0 {
            vec![Rational::one()]
        } else {
            vec![Rational::from_ratio(3, 1, &mut ledger()?)?, Rational::one()]
        };
        assert_eq!(
            replacement
                .current(&fixture.heap, &mut ledger()?)?
                .indices(),
            expected
        );
        retired(&fixture)?;
    }
    Ok(())
}
