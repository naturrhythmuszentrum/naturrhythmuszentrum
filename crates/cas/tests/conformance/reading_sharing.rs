// SPDX-License-Identifier: AGPL-3.0-only
//! Shared identical source checks preserve original syntax, context and replay refusals.

use crate::testing::common::{TestResult, real};
use crate::testing::readings::{Fixture, ledger, map, state_case};
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, InvalidCode, Rational};

/// Identical endpoints still traverse an unsupported original divisor beneath a zero map.
/// ´test:integration:identical-reading-keeps-invalid-source´
#[test]
fn identical_reading_keeps_invalid_source() -> TestResult {
    let mut fixture = Fixture::new(&[("source", real()), ("reading", real())])?;
    let source = fixture.symbol("source")?;
    let symbol = fixture.symbol("reading")?;
    let mut retained = ledger()?;
    let valid =
        fixture
            .cas
            .define_reading(&symbol, &source, &map(1, 1, &[0], &[0])?, &mut ledger()?)?;
    fixture
        .cas
        .prove_reading_equality(&fixture.context, &valid, &valid, &mut retained)?;
    let zero = fixture.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let quotient = fixture.term(Operator::Div, real(), &[source, zero])?;
    let original =
        fixture
            .cas
            .define_reading(&symbol, &quotient, &map(1, 1, &[0], &[0])?, &mut ledger()?)?;
    let before = fixture.cas.counts();
    let result =
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &original, &original, &mut retained);
    assert!(matches!(
        result,
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::ReadingDefinition),
            ..
        })
    ));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// A source shared inside one proof retains its complete expected claim on fresh replay.
/// ´test:integration:identical-reading-replays-complete-source´
#[test]
fn identical_reading_replays_complete_source() -> TestResult {
    let mut source = state_case(0)?;
    source.right.clone_from(&source.left);
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut receiver = state_case(0)?;
    receiver.right.clone_from(&receiver.left);
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        receiver
            .fixture
            .cas
            .export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    let mut changed = state_case(1)?;
    changed.right.clone_from(&changed.left);
    let expected = changed.expected()?;
    let before = changed.fixture.cas.counts();
    assert!(
        changed
            .fixture
            .cas
            .replay(&bytes, &expected, &mut ledger()?)
            .is_err()
    );
    assert_eq!(changed.fixture.cas.counts(), before);
    Ok(())
}

/// Repeated complete statements cite the original checked proof without repeating its derivation.
/// ´test:integration:repeated-reading-cites-original-proof´
#[test]
fn repeated_reading_cites_original_proof() -> TestResult {
    let mut source = state_case(0)?;
    let mut budget = ledger()?;
    let original = source.prove(&mut budget)?;
    let miss = budget.usage().work;
    let before = source.fixture.cas.counts();
    let repeated = source.prove(&mut budget)?;
    assert!(budget.usage().work - miss < miss);
    assert_eq!(repeated.proof(), original.proof());
    assert_eq!(repeated.judgment(), original.judgment());
    assert_eq!(source.fixture.cas.counts(), before);
    let bytes = source
        .fixture
        .cas
        .export_witness(&repeated, &mut ledger()?)?;
    assert_eq!(
        bytes,
        source
            .fixture
            .cas
            .export_witness(&original, &mut ledger()?)?
    );
    let mut receiver = state_case(0)?;
    let expected = receiver.expected()?;
    receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    Ok(())
}

/// Matching graph serials from another store never inherit an earlier statement's authority.
/// ´test:integration:reading-proof-memo-separates-source-stores´
#[test]
fn reading_proof_memo_separates_source_stores() -> TestResult {
    let mut first = state_case(0)?;
    let mut second = state_case(1)?;
    let mut budget = ledger()?;
    let original = first.prove(&mut budget)?;
    let other = second.prove(&mut budget)?;
    let first_bytes = first
        .fixture
        .cas
        .export_witness(&original, &mut ledger()?)?;
    let other_bytes = second.fixture.cas.export_witness(&other, &mut ledger()?)?;
    assert_ne!(first_bytes, other_bytes);
    let expected = second.expected()?;
    assert_eq!(other.judgment(), &expected);
    let before = second.fixture.cas.counts();
    assert!(
        second
            .fixture
            .cas
            .replay(&first_bytes, &expected, &mut ledger()?)
            .is_err()
    );
    assert_eq!(second.fixture.cas.counts(), before);
    Ok(())
}

/// A warm citation still consumes the same cumulative ledger and refuses one unit before completion.
/// ´test:integration:repeated-reading-work-is-cumulative´
#[test]
fn repeated_reading_work_is_cumulative() -> TestResult {
    let mut measured = state_case(0)?;
    let mut budget = ledger()?;
    measured.prove(&mut budget)?;
    measured.prove(&mut budget)?;
    let needed = budget.usage().work;
    for (work, success) in [(needed, true), (needed - 1, false)] {
        let mut source = state_case(0)?;
        let mut budget = nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
            work,
            ..nrz_cas::limits::Limits::default()
        })?;
        source.prove(&mut budget)?;
        let before = source.fixture.cas.counts();
        assert_eq!(source.prove(&mut budget).is_ok(), success);
        assert_eq!(source.fixture.cas.counts(), before);
    }
    Ok(())
}
