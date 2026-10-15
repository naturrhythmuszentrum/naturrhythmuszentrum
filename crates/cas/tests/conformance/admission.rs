// SPDX-License-Identifier: AGPL-3.0-only
//! Public locality and witness-preserving admission rollback.

use crate::testing::admission::Fixture;
use crate::testing::common::{TestResult, default_budget};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::syntax::Draft;
use nrz_cas::{Cas, FaultKind, InvalidCode};

/// An unrelated large source changes storage occupancy but neither operation's Work charge.
/// ´test:integration:walk-and-admission-work-locality´
#[test]
fn walk_and_admission_work_locality() -> TestResult {
    let mut small = Fixture::new(0)?;
    let mut large = Fixture::new(100_000)?;
    let mut baseline = default_budget()?;
    let mut padded = default_budget()?;
    small.cas.walk_original(&small.root, &mut baseline)?;
    large.cas.walk_original(&large.root, &mut padded)?;
    assert_eq!(baseline.usage().work, padded.usage().work);
    let mut baseline = default_budget()?;
    let mut padded = default_budget()?;
    let first = small.batch();
    let second = large.batch();
    small.cas.admit(&first, &mut baseline)?;
    large.cas.admit(&second, &mut padded)?;
    assert_eq!(baseline.usage().work, padded.usage().work);
    Ok(())
}

/// A walk's own workspace ceiling remains an explicit named resource refusal.
/// ´test:integration:walk-workspace-refusal´
#[test]
fn walk_workspace_refusal() -> TestResult {
    let fixture = Fixture::new(100_000)?;
    let mut local = Budget::new(Limits {
        staged_payload_bytes: 4_096,
        ..Limits::default()
    })?;
    fixture.cas.walk_original(&fixture.root, &mut local)?;
    assert!(fixture.cas.counts().payload_bytes > 4_096);
    let mut budget = Budget::new(Limits {
        staged_payload_bytes: 1,
        ..Limits::default()
    })?;
    let error = fixture
        .cas
        .walk_original(&fixture.root, &mut budget)
        .err()
        .ok_or("accepted tiny walk workspace")?;
    assert!(matches!(
        error.fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::StagedPayloadBytes,
            ..
        }
    ));
    assert_eq!(error.events.len(), 0);
    Ok(())
}

/// Preserve a real earlier cancellation proof across rejected graph mutations.
fn witness_store() -> TestResult<(Cas, VerifiedWitness)> {
    let (fixture, witness) = Fixture::with_witness()?;
    Ok((fixture.cas, witness))
}

/// Distinct source inserts expose rollback of interning, source keys and serial allocation.
fn sources() -> Vec<Draft> {
    [("first", 1), ("second", 1_000), ("third", 1)]
        .into_iter()
        .map(|(key, bytes)| Draft::Source {
            key: key.into(),
            text: "x".repeat(bytes),
        })
        .collect()
}

/// Invalid middle input cannot affect canonical state, prior proof bytes or the next source ID.
/// ´test:integration:invalid-admission-preserves-witness´
#[test]
fn invalid_admission_preserves_witness() -> TestResult {
    let (mut cas, witness) = witness_store()?;
    let bytes = cas.export_witness(&witness, &mut default_budget()?)?;
    let counts = cas.counts();
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let mut drafts = sources();
    drafts.insert(
        2,
        Draft::Source {
            key: "first".into(),
            text: "conflict".into(),
        },
    );
    let error = cas
        .admit(&drafts, &mut default_budget()?)
        .err()
        .ok_or("accepted conflict")?;
    assert_eq!(error.kind, FaultKind::Invalid(InvalidCode::SourceConflict));
    assert_eq!(cas.counts(), counts);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    assert_eq!(cas.export_witness(&witness, &mut default_budget()?)?, bytes);
    let (mut control, _) = witness_store()?;
    let expected = control.admit(&sources(), &mut default_budget()?)?;
    let actual = cas.admit(&sources(), &mut default_budget()?)?;
    assert_eq!(
        actual
            .iter()
            .map(nrz_cas::Handle::logical_id)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(nrz_cas::Handle::logical_id)
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// A middle-draft Work refusal leaves a previous witness and all future logical IDs unchanged.
/// ´test:integration:exhausted-admission-preserves-witness´
#[test]
fn exhausted_admission_preserves_witness() -> TestResult {
    let (mut cas, witness) = witness_store()?;
    let bytes = cas.export_witness(&witness, &mut default_budget()?)?;
    let counts = cas.counts();
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let (mut control, _) = witness_store()?;
    let mut measured = default_budget()?;
    let expected = control.admit(&sources(), &mut measured)?;
    // The tiny third source needs fewer than 256 units; exhaustion occurs in the second.
    let work = measured
        .usage()
        .work
        .checked_sub(256)
        .ok_or("fixture too small")?;
    let mut limited = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    let error = cas
        .admit(&sources(), &mut limited)
        .err()
        .ok_or("accepted exhausted batch")?;
    assert!(matches!(
        error.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(cas.counts(), counts);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    assert_eq!(cas.export_witness(&witness, &mut default_budget()?)?, bytes);
    let actual = cas.admit(&sources(), &mut default_budget()?)?;
    assert_eq!(
        actual
            .iter()
            .map(nrz_cas::Handle::logical_id)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(nrz_cas::Handle::logical_id)
            .collect::<Vec<_>>()
    );
    Ok(())
}
