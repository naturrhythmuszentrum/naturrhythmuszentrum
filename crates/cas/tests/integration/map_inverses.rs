// SPDX-License-Identifier: AGPL-3.0-only
//! Independent coefficient oracles supply fresh expected judgments for every inverse witness.

use crate::testing::common::TestResult;
use crate::testing::map_inverses::{
    constants, determinant_expected, determinant_oracle, expected, fixture, ledger, parameter_case,
    rational_case,
};
use crate::testing::readings::Fixture;
use nrz_cas::readings::ExactAffineMap;

fn portable(
    kind: u32,
    source: (Fixture, ExactAffineMap, ExactAffineMap),
    receiver: (Fixture, ExactAffineMap, ExactAffineMap),
) -> TestResult {
    let (mut source, original, _) = source;
    let inverse = source
        .cas
        .invert_reading_map(&source.context, &original, &mut ledger()?)?;
    let bytes = source
        .cas
        .export_witness(inverse.witness(), &mut ledger()?)?;
    let nonsingularity = source
        .cas
        .export_witness(inverse.nonsingularity(), &mut ledger()?)?;
    let (mut receiver, original, oracle) = receiver;
    let target = expected(&mut receiver, &original, &oracle)?;
    let replayed = receiver.cas.replay(&bytes, &target, &mut ledger()?)?;
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    let value = determinant_oracle(&mut receiver, kind)?;
    let determinant = determinant_expected(&mut receiver, &original, &value)?;
    receiver
        .cas
        .replay(&nonsingularity, &determinant, &mut ledger()?)?;
    Ok(())
}

/// Every rational shape, including the empty identity, replays against an independent oracle.
/// ´test:integration:map-inverse-witnesses-replay-fresh´
#[test]
fn map_inverse_witnesses_replay_fresh() -> TestResult {
    for kind in 0..5 {
        portable(kind, rational_case(kind)?, rational_case(kind)?)?;
    }
    portable(5, parameter_case(true)?, parameter_case(true)?)
}

fn pi_case() -> TestResult<(Fixture, ExactAffineMap, ExactAffineMap)> {
    let mut case = fixture(&[])?;
    let pi = nrz_cas::ExactConstant::named(nrz_cas::syntax::Constant::Pi, &mut ledger()?)?;
    let degrees = constants(&["180"])?.pop().ok_or("degree scale")?;
    let scale = pi.div(&degrees, &mut ledger()?)?;
    let inverse = degrees.div(&pi, &mut ledger()?)?;
    let original = case.cas.constant_reading_map(
        &case.context,
        (1, 1),
        &[scale],
        &constants(&["0"])?,
        &mut ledger()?,
    )?;
    let expected = case.cas.constant_reading_map(
        &case.context,
        (1, 1),
        &[inverse],
        &constants(&["0"])?,
        &mut ledger()?,
    )?;
    Ok((case, original, expected))
}

/// The degree scale retains exact pi and its nonsingularity through fresh replay.
/// ´test:integration:pi-map-inverse-replays-exactly´
#[test]
fn pi_map_inverse_replays_exactly() -> TestResult {
    portable(6, pi_case()?, pi_case()?)
}

/// Dropping a positivity premise changes the expected whole-family claim and rejects replay.
/// ´test:integration:map-inverse-replay-retains-positive-premise´
#[test]
fn map_inverse_replay_retains_positive_premise() -> TestResult {
    let (mut source, original, _) = parameter_case(true)?;
    let inverse = source
        .cas
        .invert_reading_map(&source.context, &original, &mut ledger()?)?;
    let bytes = source
        .cas
        .export_witness(inverse.witness(), &mut ledger()?)?;
    let (mut receiver, original, oracle) = parameter_case(false)?;
    let target = expected(&mut receiver, &original, &oracle)?;
    let before = receiver.cas.counts();
    assert!(
        receiver
            .cas
            .replay(&bytes, &target, &mut ledger()?)
            .is_err()
    );
    assert_eq!(receiver.cas.counts(), before);
    let nonsingularity = source
        .cas
        .export_witness(inverse.nonsingularity(), &mut ledger()?)?;
    let value = determinant_oracle(&mut receiver, 5)?;
    let target = determinant_expected(&mut receiver, &original, &value)?;
    let before = receiver.cas.counts();
    assert!(
        receiver
            .cas
            .replay(&nonsingularity, &target, &mut ledger()?)
            .is_err()
    );
    assert_eq!(receiver.cas.counts(), before);
    Ok(())
}

/// Inverting a composed translated map equals composing inverses in reverse order.
/// ´test:integration:composed-map-inverse-order-replays´
#[test]
fn composed_map_inverse_order_replays() -> TestResult {
    let mut cases = Vec::new();
    for _ in 0..2 {
        let (mut case, first, _) = rational_case(2)?;
        let second = crate::testing::map_inverses::constant_map(
            &mut case,
            2,
            &["1", "2", "0", "1"],
            &["3", "4"],
        )?;
        let composition = case
            .cas
            .compose_reading_maps(&first, &second, &mut ledger()?)?;
        let total = case
            .cas
            .invert_reading_map(&case.context, &composition, &mut ledger()?)?;
        let outer = case
            .cas
            .invert_reading_map(&case.context, &first, &mut ledger()?)?;
        let inner = case
            .cas
            .invert_reading_map(&case.context, &second, &mut ledger()?)?;
        let reversed = case
            .cas
            .compose_reading_maps(inner.map(), outer.map(), &mut ledger()?)?;
        cases.push(crate::testing::readings::ReadingCase {
            fixture: case,
            left: total.map().expression().clone(),
            right: reversed.expression().clone(),
        });
    }
    let mut receiver = cases.pop().ok_or("receiver")?;
    let mut source = cases.pop().ok_or("source")?;
    let mut proof = ledger()?;
    let witness = source.prove(&mut proof)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let target = receiver.expected()?;
    let mut replay = ledger()?;
    receiver.fixture.cas.replay(&bytes, &target, &mut replay)?;
    assert_eq!(
        [
            proof.usage().work,
            replay.usage().work,
            u64::try_from(bytes.len())?
        ],
        [178_030, 243_738, 1297]
    );
    Ok(())
}

/// The rational convenience result carries the same portable native inverse claim.
/// ´test:integration:rational-inverse-witness-replays-fresh´
#[test]
fn rational_inverse_witness_replays_fresh() -> TestResult {
    let original = crate::testing::readings::map(2, 2, &[2, 3, 5, 7], &[11, 13])?;
    let inverse = original.inverse(&mut ledger()?)?;
    let (mut receiver, original, oracle) = rational_case(2)?;
    let target = expected(&mut receiver, &original, &oracle)?;
    receiver
        .cas
        .replay(inverse.witness_bytes(), &target, &mut ledger()?)?;
    let value = determinant_oracle(&mut receiver, 2)?;
    let target = determinant_expected(&mut receiver, &original, &value)?;
    receiver
        .cas
        .replay(inverse.nonsingularity_bytes(), &target, &mut ledger()?)?;
    Ok(())
}

fn measurement_case(kind: u32) -> TestResult<(Fixture, ExactAffineMap, ExactAffineMap)> {
    match kind {
        5 => parameter_case(true),
        6 => pi_case(),
        _ => rational_case(kind),
    }
}

fn witness_cost(
    source: &mut Fixture,
    witness: &nrz_cas::proof::VerifiedWitness,
    receiver: &mut Fixture,
    target: &nrz_cas::proof::Judgment,
    rule: nrz_cas::proof::Primitive,
) -> TestResult<[u64; 3]> {
    let step = nrz_cas::proof::ApplicationDraft {
        rule,
        context: source.context.clone(),
        conclusion: witness.judgment().statement.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: nrz_cas::proof::Certificate::None,
    };
    let mut proof = ledger()?;
    source
        .cas
        .check_proof(&source.context, &[step], &mut proof)?;
    let bytes = source.cas.export_witness(witness, &mut ledger()?)?;
    let mut replay = ledger()?;
    receiver.cas.replay(&bytes, target, &mut replay)?;
    Ok([
        proof.usage().work,
        replay.usage().work,
        u64::try_from(bytes.len())?,
    ])
}

/// Capture independent kernel-check versus fresh replay costs for both inverse certificates.
/// ´test:integration:inverse-resource-measurements´
#[test]
fn inverse_resource_measurements() -> TestResult {
    let mut rows = Vec::new();
    for kind in 0..7 {
        let (mut source, original, _) = measurement_case(kind)?;
        let mut generation = ledger()?;
        let inverse = source
            .cas
            .invert_reading_map(&source.context, &original, &mut generation)?;
        let (mut receiver, original, _) = measurement_case(kind)?;
        let value = determinant_oracle(&mut receiver, kind)?;
        let target = determinant_expected(&mut receiver, &original, &value)?;
        let determinant = witness_cost(
            &mut source,
            inverse.nonsingularity(),
            &mut receiver,
            &target,
            nrz_cas::proof::Primitive::RationalIdentity,
        )?;
        let (mut receiver, original, oracle) = measurement_case(kind)?;
        let target = expected(&mut receiver, &original, &oracle)?;
        let identity = witness_cost(
            &mut source,
            inverse.witness(),
            &mut receiver,
            &target,
            nrz_cas::proof::Primitive::Jets,
        )?;
        rows.push([
            u64::from(kind),
            generation.usage().work,
            determinant[0],
            determinant[1],
            determinant[2],
            identity[0],
            identity[1],
            identity[2],
        ]);
    }
    assert_eq!(
        rows,
        [
            [0, 265_646, 60_103, 93_401, 721, 132_982, 178_887, 947],
            [1, 684_204, 101_605, 140_068, 799, 306_622, 375_160, 1_281],
            [2, 344_890, 74_583, 117_999, 921, 177_801, 240_505, 1_247],
            [3, 254_267, 55_874, 84_287, 621, 126_703, 164_764, 797],
            [4, 58_319, 25_648, 46_453, 490, 23_441, 41_652, 456],
            [5, 159_800, 48_453, 78_766, 670, 77_361, 117_216, 853],
            [6, 176_751, 54_914, 85_901, 688, 82_540, 125_944, 921],
        ]
    );
    Ok(())
}
