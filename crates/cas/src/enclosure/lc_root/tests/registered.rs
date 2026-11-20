// SPDX-License-Identifier: AGPL-3.0-only
//! Registered construction, independent receiving-store replay and atomic refusal controls.

use super::super::service::Certificate;
use super::*;

/// A durable receipt retains the entire conditional inventory and replays into fresh originals.
/// ´test:unit:lc-root-registered-replay´
/// ´claim:cas:lc-root-registered-replay´
#[test]
fn lc_root_registered_replay() -> TestResult {
    let mut source = fixture()?;
    let original = source.request.clone();
    let before = source.cas.counts();
    let receipt = source.cas.construct_lc_root(&original, &mut ledger()?)?;
    assert_eq!(receipt.request(&source.cas)?, &original);
    assert!(receipt.request(&source.cas.full_copy_for_test()).is_err());
    assert_eq!(receipt.formal_schema(), &SCHEMA);
    assert!(super::super::super::ENCLOSURE_SCHEMAS.contains(&SCHEMA));
    assert_eq!(source.cas.retained_judgment_count(), 1);
    assert_eq!(
        source.cas.counts().edges - before.edges,
        original.handles().count() + 1
    );
    let certificate = receipt.certificate(&source.cas, &mut ledger()?)?;
    let portable = certificate.to_bytes(&mut ledger()?)?;
    golden(&portable);
    let proposal = Certificate::from_bytes(&portable, &mut ledger()?)?;
    super::super::service::certificate_allocation_control(certificate);
    let mut receiver = fixture()?;
    assert!(receipt.request(&receiver.cas).is_err());
    let checked = receiver
        .cas
        .check_lc_root(&receiver.request, &proposal, &mut ledger()?)?;
    assert_eq!(checked.request(&receiver.cas)?, &receiver.request);
    assert_eq!(
        checked.certificate(&receiver.cas, &mut ledger()?)?,
        proposal
    );
    let again = receiver
        .cas
        .check_lc_root(&receiver.request, &proposal, &mut ledger()?)?;
    assert_eq!(again.request(&receiver.cas)?, &receiver.request);
    assert_eq!(receiver.cas.retained_judgment_count(), 1);
    Ok(())
}

/// Registered replay refuses altered law fields and other receipt categories without publishing.
/// ´test:unit:lc-root-registered-mutations´
/// ´claim:cas:lc-root-registered-mutations´
#[test]
fn lc_root_registered_mutations() -> TestResult {
    let mut f = fixture()?;
    let scope = super::super::wire::scope(&f.cas, &f.request, &mut ledger()?)?;
    for mutation in 0..7 {
        let mut candidate = proposed(&f)?;
        match mutation {
            0 => candidate.conclusions[3].2 = Term::Reciprocal,
            1 => candidate.conclusions[1] = Predicate(Term::Root, Relation::Less, Term::Zero),
            2 => candidate.conclusions.swap(3, 5),
            3 => candidate.premises[2].1 = Relation::AtMost,
            4 => candidate.recipe = "positive-root-sensitivity",
            5 => candidate.recipe = "lc-root-ground-witness",
            _ => candidate.recipe = "enclosure.root-grid",
        }
        let wire = super::super::wire::encode(&candidate, &scope, &mut ledger()?)?;
        let certificate = Certificate::from_bytes(&wire, &mut ledger()?)?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let error = f
            .cas
            .check_lc_root(&f.request, &certificate, &mut ledger()?)
            .err()
            .ok_or("accepted mutation")?;
        assert!(matches!(error.kind, crate::FaultKind::Invalid(_)));
        assert_eq!(f.cas.retained_judgment_count(), 0);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    framing(&mut f, &scope)?;
    Ok(())
}

/// The registered entry reauthenticates foreign and modified original expressions.
/// ´test:unit:lc-root-registered-originals´
/// ´claim:cas:lc-root-registered-originals´
#[test]
fn lc_root_registered_originals() -> TestResult {
    let mut source = fixture()?;
    let receipt = source
        .cas
        .construct_lc_root(&source.request, &mut ledger()?)?;
    let certificate = receipt.certificate(&source.cas, &mut ledger()?)?;
    let mut f = fixture()?;
    assert!(
        f.cas
            .check_lc_root(&source.request, &certificate, &mut ledger()?)
            .is_err()
    );
    for mutation in 0..3 {
        let mut changed = f.request.clone();
        match mutation {
            0 => changed.targets[1] = changed.targets[2].clone(),
            1 => {
                changed.targets[1] = term(
                    &mut f.cas,
                    &f.header,
                    Operator::Neg,
                    real(),
                    &[changed.targets[1].clone()],
                )?;
            }
            _ => changed.targets.swap(0, 1),
        }
        let error = f
            .cas
            .check_lc_root(&changed, &certificate, &mut ledger()?)
            .err()
            .ok_or("changed target accepted")?;
        assert!(matches!(error.kind, crate::FaultKind::Invalid(_)));
        assert_eq!(f.cas.retained_judgment_count(), 0);
    }
    let root = f.cas.checked(&f.request.targets[1])?;
    let exponent = f.cas.state.expression(root)?.operands[1];
    let index = f.cas.state.indices[&exponent];
    let crate::graph::Vertex::Expression(value) = &mut f.cas.state.graph[index] else {
        return Err("exponent".into());
    };
    value.operator = Operator::Rational(Rational::from_ratio(1, 3, &mut ledger()?)?);
    let error = f
        .cas
        .check_lc_root(&f.request, &certificate, &mut ledger()?)
        .err()
        .ok_or("modified exponent accepted")?;
    assert!(matches!(error.kind, crate::FaultKind::Invalid(_)));
    assert_eq!(f.cas.retained_judgment_count(), 0);
    Ok(())
}

/// Literal full lifecycle event lists, Work and byte neighbours preserve original caller usage.
/// ´test:unit:lc-root-registered-boundaries´
/// ´claim:cas:lc-root-registered-boundaries´
#[test]
fn lc_root_registered_boundaries() -> TestResult {
    let mut f = fixture()?;
    assert_eq!(f.cas.counts().payload_bytes, 11_981);
    let mut build = ledger()?;
    let (result, build_rows) =
        crate::testing::work_trace::measured(|| f.cas.construct_lc_root(&f.request, &mut build))?;
    let receipt = result?;
    assert_eq!(f.cas.retained_judgment_bytes(), 8110);
    let certificate = receipt.certificate(&f.cas, &mut ledger()?)?;
    let mut receiver = fixture()?;
    let mut replay = ledger()?;
    let (result, replay_rows) = crate::testing::work_trace::measured(|| {
        receiver
            .cas
            .check_lc_root(&receiver.request, &certificate, &mut replay)
    })?;
    result?;
    assert_eq!(receiver.cas.retained_judgment_bytes(), 8086);
    assert_eq!(
        (build.usage().work, replay.usage().work),
        (173_200, 187_375)
    );
    let events = |rows: Vec<crate::testing::work_trace::Row>| {
        rows.into_iter()
            .map(|row| (row.location, row.work))
            .collect::<Vec<_>>()
    };
    let expected = |rows: &[(&str, u64)]| {
        rows.iter()
            .map(|(name, work)| ((*name).to_owned(), *work))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        events(build_rows),
        expected(&[
            ("LC root family checker", 13_881),
            ("candidate", 163),
            ("checking", 135),
            ("encoding", 78),
            ("lookup", 7039),
            ("originals", 61),
            ("proof transaction", 256),
            ("publication", 8128),
            ("rational encoding", 128),
            ("typed witness DAG", 137_965),
            ("typed witness record", 5366),
        ])
    );
    assert_eq!(
        events(replay_rows),
        expected(&[
            ("LC root family checker", 13_881),
            ("candidate", 18),
            ("checking", 135),
            ("encoding", 78),
            ("lookup", 7039),
            ("originals", 61),
            ("proof transaction", 7292),
            ("publication", 8104),
            ("rational encoding", 128),
            ("typed witness DAG", 137_965),
            ("typed witness record", 5366),
            ("witness decoding", 7308),
        ])
    );
    // G=11_981, checkpoint=5*64, request=504, wire=13_794+24, receipt=32.
    // Transfer: candidate888 + 2*7038 key buffers + 16 Arc counts + 24 Vec + 16 Arc.
    // Append: retained8110 + (18*4+24) dependency buffer. Transfer is the larger peak.
    // Replay retains the supplied 7036+24 bytes and reserves one fewer binder handle (24).
    boundaries(&certificate, 0, 173_200, 41_675)?;
    // The receiving service reserves this getter, not a separately copied buffer.
    assert_eq!(certificate.retained_bytes(), 7036 + 24);
    boundaries(
        &certificate,
        1,
        187_375,
        41_675 + certificate.retained_bytes() - 24,
    )?;
    rollback()?;
    shared_ledger()?;
    Ok(())
}

/// Fresh attempts preserve the caller prefix and publish only at the exact admitted boundary.
fn boundaries(certificate: &Certificate, mode: usize, work: u64, bytes: u64) -> TestResult {
    for (kind, exact) in [
        (BudgetKind::Work, work),
        (BudgetKind::StagedPayloadBytes, bytes),
    ] {
        for admitted in [false, true] {
            let mut f = fixture()?;
            let before = f.cas.canonical_snapshot(&mut ledger()?)?;
            let limit = exact - u64::from(!admitted);
            let limits = if kind == BudgetKind::Work {
                Limits {
                    work: limit + 7,
                    ..Limits::default()
                }
            } else {
                Limits {
                    staged_payload_bytes: limit + 37,
                    ..Limits::default()
                }
            };
            let mut budget = Budget::new(limits)?;
            budget.work(7, AT)?;
            let result = budget.with_retained_payload(37, AT, |budget| {
                if mode == 0 {
                    f.cas.construct_lc_root(&f.request, budget)
                } else {
                    f.cas.check_lc_root(&f.request, certificate, budget)
                }
            });
            if admitted {
                assert_eq!(result?.request(&f.cas)?, &f.request);
                assert_eq!(budget.usage().work, work + 7);
                assert_eq!(f.cas.retained_judgment_count(), 1);
            } else {
                let error = result.err().ok_or("one-below accepted")?;
                assert!(
                    matches!(error.kind, crate::FaultKind::Exhausted { budget: actual, .. } if actual == kind)
                );
                assert!(budget.usage().work > 7);
                assert_eq!(f.cas.retained_judgment_count(), 0);
                assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
            if kind == BudgetKind::StagedPayloadBytes {
                assert_eq!(budget.limits().staged_payload_bytes, limit + 37);
            }
        }
    }
    Ok(())
}

/// A later enclosing refusal invalidates a returned inner receipt without refunding its Work.
fn rollback() -> TestResult {
    let mut f = fixture()?;
    let request = f.request.clone();
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let counts = f.cas.counts();
    let refusal = Fault::invalid(InvalidCode::Policy, AT);
    let mut receipt = None;
    let mut budget = ledger()?;
    let result: Result<(), Fault> = f.cas.transaction(&mut budget, |cas, budget| {
        receipt = Some(cas.construct_lc_root(&request, budget)?);
        Err(refusal.clone())
    });
    assert_eq!(result, Err(refusal));
    assert!(budget.usage().work > 7);
    assert_eq!(f.cas.counts(), counts);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    assert!(
        receipt
            .ok_or("no completed inner receipt")?
            .request(&f.cas)
            .is_err()
    );
    Ok(())
}

/// A pre-existing caller registration grows once when another root judgment is published.
fn shared_ledger() -> TestResult {
    let mut f = fixture()?;
    let mut budget = ledger()?;
    f.cas.construct_lc_root(&f.request, &mut budget)?;
    crate::decision_kernel::cache::retained(&f.cas, &mut budget, |_| Ok(()))?;
    let old = f.cas.retained_judgment_bytes();
    assert_eq!(budget.checked_records.bytes, old + 64);
    let mut another = f.request.clone();
    another.positions[0] = 3;
    f.cas.construct_lc_root(&another, &mut budget)?;
    assert_eq!(f.cas.retained_judgment_count(), 2);
    assert!(f.cas.retained_judgment_bytes() > old);
    assert_eq!(
        budget.checked_records.bytes,
        f.cas.retained_judgment_bytes() + 64
    );
    Ok(())
}

/// New wrapper bytes are fixed independently of the existing original-syntax grammar.
fn golden(bytes: &[u8]) {
    assert_eq!(bytes.len(), 7036);
    assert_eq!(&bytes[..8], b"NRZLCR01");
    assert_eq!(&bytes[8..12], &28_u32.to_le_bytes());
    assert_eq!(&bytes[12..40], b"lc-positive-root-composition");
    assert_eq!(&bytes[40..48], &1_u64.to_le_bytes());
    assert_eq!(&bytes[48..52], &6845_u32.to_le_bytes());
    assert_eq!(&bytes[52..60], &1_u64.to_le_bytes());
    assert_eq!(&bytes[6897..6901], &9_u32.to_le_bytes());
    for (slot, expected) in [1_u32, 2, 3, 8, 9, 10, 11, 12, 13].into_iter().enumerate() {
        assert_eq!(
            &bytes[6901 + 4 * slot..6905 + 4 * slot],
            &expected.to_le_bytes()
        );
    }
    assert_eq!(
        &bytes[6937..],
        &[
            0, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 0, 7, 0, 7, 0, 1, 3, 0, 3, 0, 1, 7, 1, 0, 0, 0, 7,
            2, 0, 0, 1, 7, 3, 8, 2, 1, 7, 0, 7, 1, 1, 8, 3, 7, 4, 1, 9, 3, 9, 2, 1, 7, 5, 0, 0, 0,
            3, 0, 0, 0, 0, 4, 0, 6, 0, 2, 0, 0, 7, 2, 1, 4, 0, 4, 0, 1, 7, 3, 7, 4, 1, 5, 0, 5, 0,
            1, 7, 5, 0, 1, 2, 3, 0, 1, 2, 3, 4,
        ]
    );
}

/// Unknown versions and malformed framing cannot bypass the typed registered checker.
fn framing(f: &mut Fixture, scope: &[u8]) -> TestResult {
    let original = super::super::wire::encode(&proposed(f)?, scope, &mut ledger()?)?;
    for offset in [40, 52] {
        let mut changed = original.clone();
        changed[offset] = 2;
        let certificate = Certificate::from_bytes(&changed, &mut ledger()?)?;
        let error = f
            .cas
            .check_lc_root(&f.request, &certificate, &mut ledger()?)
            .err()
            .ok_or("unknown version accepted")?;
        assert_eq!(
            error.kind,
            crate::FaultKind::Invalid(InvalidCode::UnsupportedVersion)
        );
    }
    let mut trailing = original;
    trailing.push(0);
    let certificate = Certificate::from_bytes(&trailing, &mut ledger()?)?;
    assert!(
        f.cas
            .check_lc_root(&f.request, &certificate, &mut ledger()?)
            .is_err()
    );
    assert_eq!(f.cas.retained_judgment_count(), 0);
    Ok(())
}
