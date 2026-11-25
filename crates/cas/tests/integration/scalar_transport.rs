// SPDX-License-Identifier: AGPL-3.0-only
//! Portable guarded conversion preserves independently stated residuals.

use crate::testing::budgets::proof_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::scalar_transport::chain;

#[path = "scalar_transport/frames.rs"]
mod frames;

/// Replay separately budgets live storage, traversal depth and condition visits through nested transport.
/// Replay depth 1/2/3 adds 12+2+16+16=46, 20+2+24+24=70,
/// and 28+2+32+32=94 local-entry Work above unchanged operation Work. Refusal prefixes
/// account only for entries reached before their typed limit; the commit
/// ledger expands each row. Old/new interning lookup floors match.
/// ´test:integration:scalar-transport-replay-nonwork-boundaries´
#[test]
fn scalar_transport_replay_nonwork_boundaries() -> TestResult {
    use crate::testing::scalar_transport_work::{Operation, bounded_attempt};
    use nrz_cas::limits::BudgetKind::{ConditionSteps, Depth, StagedPayloadBytes};
    // Fresh complete replays retain 37 prior Work and 19 caller bytes; failures consume prefixes.
    for (depth, work, rows) in [
        (
            1,
            104_995,
            [
                (StagedPayloadBytes, 24_435, 98_729),
                (Depth, 3, 58_474),
                (ConditionSteps, 65, 104_705),
            ],
        ),
        (
            2,
            140_803,
            [
                (StagedPayloadBytes, 32_227, 131_062),
                (Depth, 4, 80_833),
                (ConditionSteps, 102, 140_513),
            ],
        ),
        (
            3,
            179_239,
            [
                (StagedPayloadBytes, 40_019, 165_244),
                (Depth, 5, 105_041),
                (ConditionSteps, 149, 178_949),
            ],
        ),
    ] {
        for (resource, amount, prefix) in rows {
            assert_eq!(
                bounded_attempt(depth, Operation::Replay, resource, amount)?,
                (true, work)
            );
            assert_eq!(
                bounded_attempt(depth, Operation::Replay, resource, amount - 1)?,
                (false, prefix)
            );
        }
    }
    Ok(())
}

/// Multiple restrictions convert nested meets without discharging the nonzero residual.
/// ´test:integration:scalar-transport-residual-replay´
#[test]
fn scalar_transport_residual_replay() -> TestResult {
    for depth in [0, 1, 3] {
        let mut source = chain(false, depth)?;
        let context = source.frames.last().ok_or("missing frame")?;
        let witness = source
            .fixture
            .cas
            .check_proof(context, &source.steps, &mut budget()?)?;
        assert_eq!(witness.judgment(), &source.expected);
        let bytes = source
            .fixture
            .cas
            .export_witness(&witness, &mut budget()?)?;
        for reordered in [false, true] {
            for mutation in 0..if depth == 0 { 4 } else { 6 } {
                let mut receiver = chain(reordered, depth)?;
                let mut expected = receiver.expected.clone();
                match mutation {
                    0 => expected.residuals.clear(),
                    1 => {
                        expected.residuals[0].predicate = receiver.fixture.predicate(
                            nrz_cas::syntax::PredicateKind::Defined,
                            &[receiver.fixture.x.clone()],
                        )?;
                    }
                    2 => {
                        if let nrz_cas::proof::Statement::Equality { right, .. } =
                            &mut expected.statement
                        {
                            *right = receiver.fixture.zero.clone();
                        }
                    }
                    3 => {
                        expected.residuals[0].predicate = receiver.fixture.predicate(
                            nrz_cas::syntax::PredicateKind::NonZero,
                            &[receiver.fixture.one.clone()],
                        )?;
                    }
                    4 => expected.residuals[0].home = receiver.fixture.context.clone(),
                    _ => expected.residuals[0].region = receiver.fixture.domain.clone(),
                }
                let before = receiver.fixture.cas.canonical_snapshot(&mut budget()?)?;
                assert!(
                    receiver
                        .fixture
                        .cas
                        .replay(&bytes, &expected, &mut budget()?)
                        .is_err()
                );
                assert_eq!(
                    receiver.fixture.cas.canonical_snapshot(&mut budget()?)?,
                    before
                );
                let replayed =
                    receiver
                        .fixture
                        .cas
                        .replay(&bytes, &receiver.expected, &mut budget()?)?;
                assert_eq!(replayed.judgment(), &receiver.expected);
                assert_eq!(
                    receiver
                        .fixture
                        .cas
                        .export_witness(&replayed, &mut budget()?)?,
                    bytes
                );
            }
        }
    }
    Ok(())
}

/// Complete portable records retain every weakening subset and scalar consumer dependency.
/// ´test:integration:scalar-transport-wire-links´
#[test]
fn scalar_transport_wire_links() -> TestResult {
    use crate::testing::answer::{index, records};
    let mut source = chain(false, 3)?;
    let witness =
        source
            .fixture
            .cas
            .check_proof(&source.frames[3], &source.steps, &mut budget()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut budget()?)?;
    let inventory = records(&bytes)?;
    let mut mutations = 0;
    for (_, application) in inventory.iter().filter(|(tag, _)| *tag == 11) {
        assert_eq!(
            bytes.get(application.start..application.start + 4),
            Some(1_u32.to_le_bytes().as_slice())
        );
        assert!(matches!(bytes.get(application.start + 4), Some(0 | 1)));
        assert_eq!(bytes.get(application.start + 5), Some(&0));
        let count = index(&bytes, &mut (application.start + 14))?;
        assert_eq!(
            index(&bytes, &mut (application.start - 4))?,
            u32::try_from(application.len())?
        );
        for slot in 0..count {
            let offset = application.start + 18 + 4 * usize::try_from(slot)?;
            let mut cursor = offset;
            let premise = usize::try_from(index(&bytes, &mut cursor)?)?;
            assert_eq!(inventory.get(premise).ok_or("proof record")?.0, 11);
            let mut changed = bytes.clone();
            changed
                .get_mut(application.start + 14..application.start + 18)
                .ok_or("count")?
                .copy_from_slice(&(count - 1).to_le_bytes());
            changed.drain(offset..offset + 4);
            changed
                .get_mut(application.start - 4..application.start)
                .ok_or("length")?
                .copy_from_slice(&u32::try_from(application.len() - 4)?.to_le_bytes());
            assert_eq!(records(&changed)?.len(), inventory.len());
            for reordered in [false, true] {
                let mut receiver = chain(reordered, 3)?;
                let before = receiver.fixture.cas.canonical_snapshot(&mut budget()?)?;
                assert!(
                    receiver
                        .fixture
                        .cas
                        .replay(&changed, &receiver.expected, &mut budget()?)
                        .is_err()
                );
                assert_eq!(
                    receiver.fixture.cas.canonical_snapshot(&mut budget()?)?,
                    before
                );
                receiver
                    .fixture
                    .cas
                    .replay(&bytes, &receiver.expected, &mut budget()?)?;
                mutations += 1;
            }
        }
    }
    assert_eq!(mutations, 14);
    Ok(())
}

/// Export prepays its byte copy; replay consumes the measured prefix before atomic refusal.
/// Export performs no receiving-store map edit and stays at unchanged operation Work. Replay depth
/// 0/1/3 adds 4+2+8+8=22, 12+2+16+16=46, 28+2+32+32=94 Work.
/// ´test:integration:scalar-transport-portable-resource-boundaries´
#[test]
fn scalar_transport_portable_resource_boundaries() -> TestResult {
    use crate::testing::scalar_transport_work::{Operation, attempt};
    // Totals include 37 prior Work and retain 19 caller payload bytes throughout.
    for (depth, export, export_prefix, replay, prefix) in [
        (0, 596, 37, 63_857, 63_780),
        (1, 932, 37, 104_995, 104_885),
        (3, 1_388, 37, 179_239, 179_129),
    ] {
        assert_eq!(attempt(depth, Operation::Export, export)?, (true, export));
        assert_eq!(
            attempt(depth, Operation::Export, export - 1)?,
            (false, export_prefix)
        );
        assert_eq!(attempt(depth, Operation::Replay, replay)?, (true, replay));
        assert_eq!(
            attempt(depth, Operation::Replay, replay - 1)?,
            (false, prefix)
        );
    }
    Ok(())
}
