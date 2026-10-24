// SPDX-License-Identifier: AGPL-3.0-only
//! Exact similarity and time normalization retain the complete two-parameter family.

use crate::testing::builders::admit;
use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{constant_map, fixture, ledger};
use crate::testing::readings::{Fixture, ReadingCase};
use nrz_cas::domain::{AssumptionDraft, ContextDraft};
use nrz_cas::readings::ExactAffineMap;
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Handle, Rational, Reference};

fn positive_family(case: &mut Fixture) -> TestResult {
    let header = case.cas.context_header(&case.context)?;
    let domain = case.cas.admissible_region(&case.context)?;
    let mut assumptions = Vec::new();
    for name in ["C", "L"] {
        let term = case.symbol(name)?;
        let predicate = admit(
            &mut case.cas,
            Draft::Condition {
                kind: PredicateKind::Positive,
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                arguments: vec![Reference::Existing(term)],
            },
            &mut ledger()?,
        )?;
        assumptions.push(AssumptionDraft {
            key: name.into(),
            condition: Reference::Existing(predicate),
        });
    }
    case.context = admit(
        &mut case.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions,
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

fn coefficient(case: &mut Fixture, number: &str, denominator: &str) -> TestResult<Handle> {
    let numerator = case.term(
        Operator::Rational(Rational::parse(number, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let denominator = case.symbol(denominator)?;
    case.term(Operator::Div, real(), &[numerator, denominator])
}

fn operator(case: &mut Fixture, top: &str, bottom: &str) -> TestResult<ExactAffineMap> {
    let zero = case.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let upper = coefficient(case, top, "C")?;
    let lower = coefficient(case, bottom, "L")?;
    let coefficients = [zero.clone(), upper, lower, zero.clone()];
    Ok(case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &coefficients,
        &[zero.clone(), zero],
        &mut ledger()?,
    )?)
}

fn normalization(change: u32) -> TestResult<ReadingCase> {
    let mut case = fixture(&["L", "C"])?;
    positive_family(&mut case)?;
    let state = constant_map(
        &mut case,
        2,
        &[if change == 2 { "10" } else { "5" }, "0", "0", "1/10"],
        &["0", "0"],
    )?;
    let inverse = case
        .cas
        .invert_reading_map(&case.context, &state, &mut ledger()?)?;
    let time = if change == 1 { "1/1000" } else { "1/2000" };
    let time = constant_map(&mut case, 2, &[time, "0", "0", time], &["0", "0"])?;
    let physical = operator(&mut case, if change == 3 { "2" } else { "1" }, "-1")?;
    let right_scaled = case
        .cas
        .compose_reading_maps(&physical, &state, &mut ledger()?)?;
    let similarity = case
        .cas
        .compose_reading_maps(inverse.map(), &right_scaled, &mut ledger()?)?;
    let normalized = case
        .cas
        .compose_reading_maps(&time, &similarity, &mut ledger()?)?;
    let upper = match change {
        1 | 3 => "1/50000",
        2 => "1/200000",
        _ => "1/100000",
    };
    let lower = if change == 1 || change == 2 {
        "-1/20"
    } else {
        "-1/40"
    };
    let direct = operator(&mut case, upper, lower)?;
    Ok(ReadingCase {
        fixture: case,
        left: normalized.expression().clone(),
        right: direct.expression().clone(),
    })
}

/// The composed exact normalization agrees with an independent coefficient formula and replays fresh.
/// ´test:integration:normalization-similarity-and-time-replay´
#[test]
fn normalization_similarity_and_time_replay() -> TestResult {
    let mut source = normalization(0)?;
    let witness = source.prove(&mut ledger()?)?;
    assert!(witness.is_discharged());
    assert_eq!(witness.hypotheses().len(), 2);
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let key = source
        .fixture
        .cas
        .witness_reuse_key(&witness, &mut ledger()?)?;
    let mut receiver = normalization(0)?;
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        key,
        receiver
            .fixture
            .cas
            .witness_reuse_key(&replayed, &mut ledger()?)?
    );
    Ok(())
}

/// Changing the time reference, state reference or coefficient changes identity and rejects the old equality.
/// ´test:integration:normalization-reference-and-operator-invalidation´
#[test]
fn normalization_reference_and_operator_invalidation() -> TestResult {
    let mut source = normalization(0)?;
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let baseline = source.fixture.cas.expression_reuse_key(
        &source.fixture.context,
        &source.left,
        &mut ledger()?,
    )?;
    for change in 1..4 {
        let mut receiver = normalization(change)?;
        let key = receiver.fixture.cas.expression_reuse_key(
            &receiver.fixture.context,
            &receiver.left,
            &mut ledger()?,
        )?;
        assert_ne!(baseline, key);
        let expected = receiver.expected()?;
        let before = receiver.fixture.cas.counts();
        assert!(
            receiver
                .fixture
                .cas
                .replay(&bytes, &expected, &mut ledger()?)
                .is_err()
        );
        assert_eq!(receiver.fixture.cas.counts(), before);
        receiver.prove(&mut ledger()?)?;
    }
    Ok(())
}

/// Exact specialization of the already checked family gives the dimensionless reference rotation.
/// ´test:integration:normalization-reference-instance-is-exact´
#[test]
fn normalization_reference_instance_is_exact() -> TestResult {
    let mut case = normalization(0)?;
    case.prove(&mut ledger()?)?;
    let mut replacements = Vec::new();
    for (key, text) in [("L", "1/40"), ("C", "1/100000")] {
        let replacement = case.fixture.term(
            Operator::Rational(Rational::parse(text, &mut ledger()?)?),
            real(),
            &[],
        )?;
        replacements.push(nrz_cas::syntax::Substitution {
            key: key.into(),
            replacement,
        });
    }
    let actual = case
        .fixture
        .cas
        .substitute(&case.left, &replacements, &mut ledger()?)?;
    let oracle = constant_map(&mut case.fixture, 2, &["0", "1", "-1", "0"], &["0", "0"])?;
    case.fixture.cas.prove_reading_equality(
        &case.fixture.context,
        &actual,
        oracle.expression(),
        &mut ledger()?,
    )?;
    Ok(())
}

/// Capture the exact family proof, fresh replay and complete-key extraction costs.
/// ´test:integration:normalization-resource-measurements´
#[test]
fn normalization_resource_measurements() -> TestResult {
    let mut source = normalization(0)?;
    let mut proof = ledger()?;
    let witness = source.prove(&mut proof)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut keys = ledger()?;
    let key = source.fixture.cas.witness_reuse_key(&witness, &mut keys)?;
    let mut receiver = normalization(0)?;
    let target = receiver.expected()?;
    let mut replay = ledger()?;
    receiver.fixture.cas.replay(&bytes, &target, &mut replay)?;
    let row = [
        proof.usage().work,
        replay.usage().work,
        u64::try_from(bytes.len())?,
        keys.usage().work,
        u64::try_from(key.as_bytes().len())?,
    ];
    assert_eq!(row, [573_160, 683_997, 1_882, 156_622, 3_705]);
    Ok(())
}
