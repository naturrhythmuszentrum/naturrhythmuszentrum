// SPDX-License-Identifier: AGPL-3.0-only
//! Widened attempts preserve archived mathematical evidence and monotone live retirement.

use super::{decision_fixture::ledger, lattice_live, lattice_selection};
use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{
    Rational,
    lattice::{LatticeEnclosure, LatticeSelectionOutcome, LatticeUndecidedReason},
};
use nrz_matrix::{Heap, HeapLimits, LiveDecisionError};

/// The guarded widened attempt adds no graph record and grants no new selected authority.
/// ´test:integration:lattice-shared-widened-attempt´
/// ´claim:matrix:lattice-shared-widened-attempt´
#[test]
fn lattice_shared_widened_attempt() -> TestResult {
    let source = lattice_selection::prepared(1, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let cover = source.cover.clone();
    let expected = source.expected.clone();
    let mut wide = source.enclosure.clone();
    widen(&mut wide)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = lattice_live::inputs(&mut heap, &association, 1)?;
    let (prepared, handle) =
        lattice_live::candidate(&heap, source, &association, &inputs, &mut ledger()?)?;
    let published = heap.publish_lattice_decision(prepared, &mut ledger()?)?;
    let archived = published.current(&heap, &mut ledger()?)?.clone();
    let descendant = published.index(&heap, 0, (0, 3), &mut ledger()?)?;
    let version = heap.value(handle)?.stamp().version();
    heap.replace_declared_bounds(handle, version, &[19.5], &[1.5])?;
    let Err(fault) = descendant.current(&heap, &mut ledger()?) else {
        panic!("widening must retire the old descendant");
    };
    assert!(matches!(fault, LiveDecisionError::Snapshot(_)));
    let mut cas = archived.cas()?;
    let before = cas.counts();
    let bytes = cas.canonical_snapshot(&mut ledger()?)?;
    let witness = cas.export_witness(archived.witness(), &mut ledger()?)?;
    let mut budget = ledger()?;
    let attempt = cas.prepare_lattice_selection(&cover, &expected, &wide, &mut budget)?;
    assert_eq!(budget.usage().work, 129_218);
    let LatticeSelectionOutcome::Undecided(undecided) =
        cas.select_lattice_attempt(&attempt, &mut budget)?
    else {
        panic!("widened box must be undecided");
    };
    assert_eq!(
        undecided.reason,
        LatticeUndecidedReason::BoundaryIntersection
    );
    assert!(std::ptr::eq(undecided.evidence, &raw const attempt));
    assert_eq!(undecided.spent_work, 298_242);
    assert_eq!(budget.usage().work, 298_242);
    assert_eq!(cas.counts(), before);
    assert_eq!(bytes.len(), 22_862);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, bytes);
    assert_eq!(
        cas.export_witness(archived.witness(), &mut ledger()?)?,
        witness
    );
    drop(cas);
    let Err(fault) = descendant.current(&heap, &mut ledger()?) else {
        panic!("undecided revalidation must not revive the old descendant");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    Ok(())
}

/// Widen the first native coordinate's certified original box and its transported lower bound.
pub(super) fn widen(value: &mut LatticeEnclosure) -> TestResult {
    let LatticeEnclosure::Original(value) = value else {
        return Err("original box required".into());
    };
    value.original.coordinates[0].lower = Rational::parse("1799/100", &mut ledger()?)?;
    value.original.coordinates[0].upper = Rational::parse("2101/100", &mut ledger()?)?;
    value.bounds[0].0 = Rational::parse("14/5", &mut ledger()?)?;
    Ok(())
}
