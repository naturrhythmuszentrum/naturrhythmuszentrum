// SPDX-License-Identifier: AGPL-3.0-only
//! Exact sign proposals are checked against an independent integer sign oracle and fresh replay.

mod boundaries;

use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use crate::testing::strict_sign::{expected, fixture};

/// All four strict-sign combinations transport through products, quotients, reciprocals and negation.
/// Each moved Work pin equals unchanged operation Work plus reached receiving-store local entries;
/// the commit ledger expands every proof and replay row. Old/new lookup
/// floors are equal, and witness bytes are unchanged.
/// ´test:integration:strict-sign-oracle-and-fresh-replay´
#[test]
fn strict_sign_oracle_and_fresh_replay() -> TestResult {
    let mut costs = Vec::new();
    for left in [-3_i8, 2] {
        for right in [-5_i8, 7] {
            let (mut source, subjects) = fixture(left.signum(), right.signum())?;
            let positive = i16::from(left) * i16::from(right) > 0;
            for (index, subject) in subjects.iter().enumerate() {
                let claim = expected(
                    &source,
                    subject,
                    if index == 3 { !positive } else { positive },
                )?;
                let mut proof_budget = ledger()?;
                let witness =
                    source
                        .cas
                        .prove_strict_sign(&source.context, subject, &mut proof_budget)?;
                assert!(witness.is_discharged());
                assert_eq!(witness.judgment(), &claim);
                let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
                let (mut receiver, originals) = fixture(left.signum(), right.signum())?;
                let target = originals.get(index).ok_or("original sign subject")?;
                let claim = expected(
                    &receiver,
                    target,
                    if index == 3 { !positive } else { positive },
                )?;
                let mut replay_budget = ledger()?;
                let replayed = receiver.cas.replay(&bytes, &claim, &mut replay_budget)?;
                costs.push((
                    left.signum(),
                    right.signum(),
                    index,
                    proof_budget.usage().work,
                    replay_budget.usage().work,
                    bytes.len(),
                ));
                assert_eq!(
                    receiver.cas.export_witness(&replayed, &mut ledger()?)?,
                    bytes
                );
            }
        }
    }
    assert_eq!(
        costs,
        vec![
            (-1, -1, 0, 27_133, 58_382, 705),
            (-1, -1, 1, 27_135, 58_382, 705),
            (-1, -1, 2, 33_094, 71_244, 846),
            (-1, -1, 3, 35_876, 77_374, 911),
            (-1, 1, 0, 25_654, 54_670, 662),
            (-1, 1, 1, 25_656, 54_670, 662),
            (-1, 1, 2, 31_613, 67_399, 803),
            (-1, 1, 3, 34_394, 73_468, 868),
            (1, -1, 0, 25_654, 54_671, 662),
            (1, -1, 1, 25_656, 54_671, 662),
            (1, -1, 2, 31_613, 67_400, 803),
            (1, -1, 3, 34_394, 73_469, 868),
            (1, 1, 0, 24_176, 50_998, 619),
            (1, 1, 1, 24_178, 50_998, 619),
            (1, 1, 2, 30_133, 63_594, 760),
            (1, 1, 3, 32_913, 69_602, 825),
        ]
    );
    for (left, right, index, generation, replay, _) in costs {
        let positive = (left == right) != (index == 3);
        boundaries::check(
            (left, right),
            boundaries::Subject::Indexed(index),
            positive,
            (generation, replay),
        )?;
    }
    Ok(())
}

/// Repeated monomial factors use the existing even and odd power rules with strict leaf evidence.
/// Positive and negative power rows have independent local-entry receipts
/// in the commit ledger; each ordinary boundary still checks N and N-1.
/// ´test:integration:strict-sign-power-oracle´
#[test]
fn strict_sign_power_oracle() -> TestResult {
    let mut costs = Vec::new();
    for factor in [-3_i32, 2] {
        for exponent in [2_u32, 3] {
            let (mut source, _) = fixture(i8::try_from(factor.signum())?, 1)?;
            let original = source.symbol("L")?;
            let subject = source.term(
                nrz_cas::syntax::Operator::IntegerPower(i64::from(exponent)),
                crate::testing::common::real(),
                &[original],
            )?;
            let mut proof_budget = ledger()?;
            let witness =
                source
                    .cas
                    .prove_strict_sign(&source.context, &subject, &mut proof_budget)?;
            let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
            let (mut receiver, _) = fixture(i8::try_from(factor.signum())?, 1)?;
            let original = receiver.symbol("L")?;
            let subject = receiver.term(
                nrz_cas::syntax::Operator::IntegerPower(i64::from(exponent)),
                crate::testing::common::real(),
                &[original],
            )?;
            let claim = expected(&receiver, &subject, factor.pow(exponent) > 0)?;
            let mut replay_budget = ledger()?;
            assert!(
                receiver
                    .cas
                    .replay(&bytes, &claim, &mut replay_budget)?
                    .is_discharged()
            );
            costs.push((
                factor.signum(),
                exponent,
                proof_budget.usage().work,
                replay_budget.usage().work,
                bytes.len(),
            ));
        }
    }
    assert_eq!(
        costs,
        vec![
            (-1, 2, 24_198, 52_266, 645),
            (-1, 3, 24_198, 52_266, 645),
            (1, 2, 22_720, 48_611, 602),
            (1, 3, 22_720, 48_611, 602),
        ]
    );
    for (sign, exponent, generation, replay, _) in costs {
        let positive = sign > 0 || exponent % 2 == 0;
        boundaries::check(
            (i8::try_from(sign)?, 1),
            boundaries::Subject::Power(exponent),
            positive,
            (generation, replay),
        )?;
    }
    Ok(())
}
