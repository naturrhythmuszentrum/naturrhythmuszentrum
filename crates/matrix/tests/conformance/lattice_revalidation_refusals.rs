// SPDX-License-Identifier: AGPL-3.0-only
//! Revalidation authenticates the question, complete original box and current native lease.

use super::{
    decision_fixture::ledger,
    lattice_revalidation,
    lattice_revalidation_live::{enclose, fixture, retired},
};
use crate::binding_testing::TestResult;
use nrz_bindings::DecisionError;
use nrz_matrix::{DecisionSnapshotError, Heap, HeapError, HeapLimits, LiveDecisionError};

/// A wider valid CAS receipt cannot borrow the independently contained narrow live box.
/// ´test:integration:lattice-live-attempt-box-identity´
/// ´claim:matrix:lattice-live-attempt-box-identity´
#[test]
fn lattice_live_attempt_box_identity() -> TestResult {
    let mut fixture = fixture(1, false)?;
    let enclosed = enclose(&fixture, &mut ledger()?)?;
    lattice_revalidation::widen(&mut fixture.evidence)?;
    let mut cas = fixture.archived.cas()?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let attempt = cas.prepare_lattice_selection(
        &fixture.cover,
        &fixture.expected,
        &fixture.evidence,
        &mut ledger()?,
    )?;
    let Err(fault) =
        fixture
            .heap
            .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut ledger()?)
    else {
        panic!("different original boxes must refuse before attempting cuts");
    };
    assert!(matches!(fault, LiveDecisionError::Enclosure));
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    drop(cas);
    retired(&fixture)?;
    Ok(())
}

/// An independent equal mathematical question cannot replace the retained attempt's handles.
/// ´test:integration:lattice-live-attempt-question-identity´
/// ´claim:matrix:lattice-live-attempt-question-identity´
#[test]
fn lattice_live_attempt_question_identity() -> TestResult {
    let fixture = fixture(1, true)?;
    let foreign = super::lattice_revalidation_live::fixture(1, true)?;
    let enclosed = enclose(&fixture, &mut ledger()?)?;
    let attempt = foreign.archived.cas()?.prepare_lattice_selection(
        &foreign.cover,
        &foreign.expected,
        &foreign.evidence,
        &mut ledger()?,
    )?;
    let mut cas = fixture.archived.cas()?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let Err(fault) =
        fixture
            .heap
            .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut ledger()?)
    else {
        panic!("foreign mathematical evidence must refuse at bindings question matching");
    };
    let LiveDecisionError::Admission(fault) = fault else {
        panic!("a foreign question must reach the physical question matcher");
    };
    assert!(matches!(*fault, DecisionError::Case));
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    drop(cas);
    retired(&fixture)?;
    Ok(())
}

/// A late source change or foreign heap refuses before a mathematical attempt can run.
/// ´test:integration:lattice-live-attempt-lease-refusals´
/// ´claim:matrix:lattice-live-attempt-lease-refusals´
#[test]
fn lattice_live_attempt_lease_refusals() -> TestResult {
    for foreign in [false, true] {
        let mut fixture = fixture(1, true)?;
        let enclosed = enclose(&fixture, &mut ledger()?)?;
        let other = Heap::new(HeapLimits::default())?;
        if !foreign {
            let input = enclosed.inputs().operands()[0].input();
            fixture.heap.replace_declared_bounds(
                input.handle(),
                input.stamp().version(),
                &[19.5],
                &[2.0],
            )?;
        }
        let receiver = if foreign { &other } else { &fixture.heap };
        let mut cas = fixture.archived.cas()?;
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        let attempt = cas.prepare_lattice_selection(
            &fixture.cover,
            &fixture.expected,
            &fixture.evidence,
            &mut ledger()?,
        )?;
        let Err(fault) =
            receiver.select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut ledger()?)
        else {
            panic!("invalid native leases must refuse before attempting cuts");
        };
        let LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(reason)) = fault else {
            panic!("a changed lease must reach the native snapshot checker");
        };
        assert_eq!(
            reason,
            if foreign {
                HeapError::ForeignHandle
            } else {
                HeapError::StaleEvidence
            }
        );
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
        drop(cas);
        retired(&fixture)?;
    }
    Ok(())
}
