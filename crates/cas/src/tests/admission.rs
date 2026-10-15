// SPDX-License-Identifier: AGPL-3.0-only
//! Crate-level locality and complete transaction restoration.

use crate::limits::{Budget, BudgetKind, Limits};
use crate::syntax::Draft;
use crate::testing::admission::Fixture;
use crate::testing::common::{TestResult, default_budget};
use crate::{FaultKind, InvalidCode};

/// Visiting all reachable records keeps the old conservative payload floor.
/// ´test:crate:walk-locality-and-full-coverage-floor´
#[test]
fn walk_locality_and_full_coverage_floor() -> TestResult {
    let small = Fixture::new(0)?;
    let large = Fixture::new(100_000)?;
    let mut baseline = default_budget()?;
    let mut padded = default_budget()?;
    small.cas.walk_original(&small.root, &mut baseline)?;
    large.cas.walk_original(&large.root, &mut padded)?;
    assert_eq!(baseline.usage().work, padded.usage().work);
    assert!(baseline.usage().work >= 2 * small.cas.counts().payload_bytes);
    assert!(large.cas.counts().payload_bytes > small.cas.counts().payload_bytes + 300_000);
    Ok(())
}

/// A new batch charges its own payload and probes, without charging unrelated records.
/// ´test:crate:admission-work-locality´
#[test]
fn admission_work_locality() -> TestResult {
    let mut small = Fixture::new(0)?;
    let mut large = Fixture::new(100_000)?;
    let mut baseline = default_budget()?;
    let mut padded = default_budget()?;
    let first = small.batch();
    let second = large.batch();
    small.cas.admit(&first, &mut baseline)?;
    large.cas.admit(&second, &mut padded)?;
    assert_eq!(baseline.usage().work, padded.usage().work);
    Ok(())
}

/// An invalid middle draft restores every internal map and serial as well as graph bytes.
/// ´test:crate:journal-invalid-middle-draft´
#[test]
fn journal_invalid_middle_draft() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let mut drafts = fixture.batch();
    drafts.insert(
        4,
        Draft::Source {
            key: "input".into(),
            text: "conflict".into(),
        },
    );
    let before = format!("{:?}", fixture.cas.state);
    let counts = fixture.cas.counts();
    let snapshot = fixture.cas.canonical_snapshot(&mut default_budget()?)?;
    let mut control = fixture.cas.fork(&mut default_budget()?)?;
    let error = fixture
        .cas
        .admit(&drafts, &mut default_budget()?)
        .err()
        .ok_or("accepted conflict")?;
    assert_eq!(error.kind, FaultKind::Invalid(InvalidCode::SourceConflict));
    assert_eq!(format!("{:?}", fixture.cas.state), before);
    assert_eq!(fixture.cas.counts(), counts);
    assert_eq!(
        fixture
            .cas
            .export_witness(&witness, &mut default_budget()?)?,
        bytes
    );
    assert_eq!(
        fixture.cas.canonical_snapshot(&mut default_budget()?)?,
        snapshot
    );
    let valid = fixture.batch();
    assert_eq!(
        fixture.cas.admit(&valid, &mut default_budget()?)?,
        control.admit(&valid, &mut default_budget()?)?
    );
    Ok(())
}

/// Exhaustion after earlier drafts mutate all indexes restores exactly the original state.
/// ´test:crate:journal-work-middle-draft´
#[test]
fn journal_work_middle_draft() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let drafts = fixture.batch();
    let mut control = fixture.cas.fork(&mut default_budget()?)?;
    let mut measured = default_budget()?;
    let expected = control.admit(&drafts, &mut measured)?;
    let before = format!("{:?}", fixture.cas.state);
    let counts = fixture.cas.counts();
    // The final empty source needs fewer than 256 units; refusal is in the large source before it.
    let work = measured
        .usage()
        .work
        .checked_sub(256)
        .ok_or("fixture too small")?;
    let mut limited = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    let error = fixture
        .cas
        .admit(&drafts, &mut limited)
        .err()
        .ok_or("accepted exhausted batch")?;
    assert!(matches!(
        error.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(format!("{:?}", fixture.cas.state), before);
    assert_eq!(fixture.cas.counts(), counts);
    assert_eq!(
        fixture
            .cas
            .export_witness(&witness, &mut default_budget()?)?,
        bytes
    );
    assert_eq!(
        fixture.cas.admit(&drafts, &mut default_budget()?)?,
        expected
    );
    Ok(())
}
