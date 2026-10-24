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
            (-1, -1, 0, 27_081, 58_330, 705),
            (-1, -1, 1, 27_081, 58_330, 705),
            (-1, -1, 2, 33_007, 71_159, 846),
            (-1, -1, 3, 35_766, 77_266, 911),
            (-1, 1, 0, 25_602, 54_618, 662),
            (-1, 1, 1, 25_602, 54_618, 662),
            (-1, 1, 2, 31_526, 67_314, 803),
            (-1, 1, 3, 34_284, 73_360, 868),
            (1, -1, 0, 25_602, 54_619, 662),
            (1, -1, 1, 25_602, 54_619, 662),
            (1, -1, 2, 31_526, 67_315, 803),
            (1, -1, 3, 34_284, 73_361, 868),
            (1, 1, 0, 24_124, 50_946, 619),
            (1, 1, 1, 24_124, 50_946, 619),
            (1, 1, 2, 30_046, 63_509, 760),
            (1, 1, 3, 32_803, 69_494, 825),
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
            (-1, 2, 24_164, 52_232, 645),
            (-1, 3, 24_164, 52_232, 645),
            (1, 2, 22_686, 48_577, 602),
            (1, 3, 22_686, 48_577, 602),
        ]
    );
    Ok(())
}
