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
    let mut control = fixture.cas.full_copy_for_test();
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
    let mut control = fixture.cas.full_copy_for_test();
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

/// All observable and private state, including physical adjacency and counters, must agree.
fn same_store(actual: &crate::Cas, expected: &crate::Cas) -> TestResult {
    assert_eq!(actual.counts(), expected.counts());
    assert_eq!(
        format!("{:?}", actual.state),
        format!("{:?}", expected.state)
    );
    assert_eq!(
        actual.canonical_snapshot(&mut default_budget()?)?,
        expected.canonical_snapshot(&mut default_budget()?)?
    );
    Ok(())
}

/// A native theorem exercises publication independently of the scalar proof path.
fn region_step(
    cas: &crate::Cas,
    context: &crate::Handle,
) -> Result<crate::proof::ApplicationDraft, crate::Fault> {
    let region = cas.admissible_region(context)?;
    Ok(crate::proof::ApplicationDraft {
        rule: crate::proof::Primitive::RegionConstruct,
        context: context.clone(),
        conclusion: crate::proof::Statement::Subset {
            subset: region.clone(),
            superset: region,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: crate::proof::Certificate::None,
    })
}

/// Exercise sources, parents, interning, policies, generated syntax and proof records together.
fn transaction_edits(
    cas: &mut crate::Cas,
    drafts: &[Draft],
    context: &crate::Handle,
    ledger: &mut Budget,
) -> Result<Vec<crate::Handle>, crate::Fault> {
    let mut handles = cas.admit(drafts, ledger)?;
    let policy = cas.pin_policy(&crate::cost::CostPolicy::default(), ledger)?;
    handles.push(cas.handle(policy));
    let number = crate::Rational::from_ratio(7, 1, ledger)?;
    let expression = cas.generated(
        context.logical_id(),
        crate::syntax::Operator::Rational(number),
        crate::testing::common::real(),
        &[],
        ledger,
    )?;
    handles.push(cas.handle(expression));
    let step = region_step(cas, context)?;
    handles.push(cas.check_proof(context, &[step], ledger)?.proof().clone());
    Ok(handles)
}

/// Deliberate refusal after successful graph edits.
const fn transaction_refusal() -> crate::Fault {
    crate::Fault::invalid(
        InvalidCode::Rule,
        crate::error::Location::new("transaction test"),
    )
}

/// Inner success keeps its undo records until the outermost decision.
/// ´test:crate:journal-outer-rollback-includes-inner-commit´
#[test]
fn journal_outer_rollback_includes_inner_commit() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let mut control = fixture.cas.full_copy_for_test();
    let drafts = fixture.batch();
    let context = &witness.judgment().context;
    let mut ledger = default_budget()?;
    let result: Result<(), crate::Fault> = fixture.cas.transaction(&mut ledger, |cas, ledger| {
        cas.transaction(ledger, |cas, ledger| {
            transaction_edits(cas, &drafts, context, ledger)
        })?;
        Err(transaction_refusal())
    });
    assert_eq!(result, Err(transaction_refusal()));
    assert!(ledger.usage().work > 512);
    same_store(&fixture.cas, &control)?;
    assert_eq!(
        transaction_edits(&mut fixture.cas, &drafts, context, &mut default_budget()?)?,
        transaction_edits(&mut control, &drafts, context, &mut default_budget()?)?
    );
    same_store(&fixture.cas, &control)
}

/// An inner refusal drops only its edits; the outer transaction can continue and commit.
/// ´test:crate:journal-inner-rollback-outer-commit´
#[test]
fn journal_inner_rollback_outer_commit() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let mut control = fixture.cas.full_copy_for_test();
    let drafts = fixture.batch();
    let context = &witness.judgment().context;
    let outer = Draft::Source {
        key: "outer".into(),
        text: "kept".into(),
    };
    let mut ledger = default_budget()?;
    fixture.cas.transaction(&mut ledger, |cas, ledger| {
        cas.admit(std::slice::from_ref(&outer), ledger)?;
        let spent = ledger.usage().work;
        let result: Result<(), crate::Fault> = cas.transaction(ledger, |cas, ledger| {
            transaction_edits(cas, &drafts, context, ledger)?;
            Err(transaction_refusal())
        });
        assert_eq!(result, Err(transaction_refusal()));
        assert!(ledger.usage().work > spent);
        Ok(())
    })?;
    control.admit(&[outer], &mut default_budget()?)?;
    same_store(&fixture.cas, &control)?;
    assert_eq!(
        transaction_edits(&mut fixture.cas, &drafts, context, &mut default_budget()?)?,
        transaction_edits(&mut control, &drafts, context, &mut default_budget()?)?
    );
    same_store(&fixture.cas, &control)
}

/// A real admission failure propagates through several nested checkpoints without losing a log.
/// ´test:crate:journal-deep-failure-restores-all-indexes´
#[test]
fn journal_deep_failure_restores_all_indexes() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let mut control = fixture.cas.full_copy_for_test();
    let drafts = fixture.batch();
    let context = &witness.judgment().context;
    let mut ledger = default_budget()?;
    let result = fixture.cas.transaction(&mut ledger, |cas, ledger| {
        transaction_edits(cas, &drafts, context, ledger)?;
        cas.transaction(ledger, |cas, ledger| {
            cas.transaction(ledger, |cas, ledger| {
                cas.admit(
                    &[Draft::Source {
                        key: "deep".into(),
                        text: "discarded".into(),
                    }],
                    ledger,
                )?;
                cas.admit(
                    &[Draft::Source {
                        key: "input".into(),
                        text: "conflict".into(),
                    }],
                    ledger,
                )
            })
        })
    });
    assert_eq!(
        result.err().ok_or("accepted conflict")?.kind,
        FaultKind::Invalid(InvalidCode::SourceConflict)
    );
    assert!(ledger.usage().work > 768);
    same_store(&fixture.cas, &control)?;
    assert_eq!(
        transaction_edits(&mut fixture.cas, &drafts, context, &mut default_budget()?)?,
        transaction_edits(&mut control, &drafts, context, &mut default_budget()?)?
    );
    same_store(&fixture.cas, &control)
}

/// A refusal after native proof insertion restores moved state and the next proof serial.
/// ´test:crate:journal-proof-publication-refusal´
#[test]
fn journal_proof_publication_refusal() -> TestResult {
    let (mut fixture, witness) = Fixture::with_witness()?;
    let mut control = fixture.cas.full_copy_for_test();
    let context = &witness.judgment().context;
    let step = region_step(&fixture.cas, context)?;
    let mut ledger = Budget::new(Limits {
        proof_nodes: 1,
        ..Limits::default()
    })?;
    let fault = fixture
        .cas
        .check_proof(context, std::slice::from_ref(&step), &mut ledger)
        .err()
        .ok_or("accepted second proof")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ProofNodes,
            ..
        }
    ));
    same_store(&fixture.cas, &control)?;
    let actual =
        fixture
            .cas
            .check_proof(context, std::slice::from_ref(&step), &mut default_budget()?)?;
    let expected = control.check_proof(context, &[step], &mut default_budget()?)?;
    assert_eq!(actual.proof(), expected.proof());
    same_store(&fixture.cas, &control)
}
