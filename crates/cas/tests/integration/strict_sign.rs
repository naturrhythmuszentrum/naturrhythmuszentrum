// SPDX-License-Identifier: AGPL-3.0-only
//! Exact sign proposals are checked against an independent integer sign oracle and fresh replay.

use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use crate::testing::strict_sign::{expected, fixture};

/// All four strict-sign combinations transport through products, quotients, reciprocals and negation.
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
            (-1, -1, 0, 27_752, 60_435, 705),
            (-1, -1, 1, 27_752, 60_435, 705),
            (-1, -1, 2, 34_392, 75_066, 846),
            (-1, -1, 3, 37_241, 81_807, 911),
            (-1, 1, 0, 26_196, 56_102, 662),
            (-1, 1, 1, 26_196, 56_102, 662),
            (-1, 1, 2, 32_745, 70_511, 803),
            (-1, 1, 3, 35_593, 77_191, 868),
            (1, -1, 0, 26_145, 56_052, 662),
            (1, -1, 1, 26_145, 56_052, 662),
            (1, -1, 2, 32_694, 70_461, 803),
            (1, -1, 3, 35_542, 77_141, 868),
            (1, 1, 0, 24_590, 51_758, 619),
            (1, 1, 1, 24_590, 51_758, 619),
            (1, 1, 2, 31_048, 65_945, 760),
            (1, 1, 3, 33_895, 72_564, 825),
        ]
    );
    Ok(())
}

/// Repeated monomial factors use the existing even and odd power rules with strict leaf evidence.
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
            (-1, 2, 24_768, 53_438, 645),
            (-1, 3, 24_768, 53_438, 645),
            (1, 2, 23_152, 49_101, 602),
            (1, 3, 23_152, 49_101, 602),
        ]
    );
    Ok(())
}
