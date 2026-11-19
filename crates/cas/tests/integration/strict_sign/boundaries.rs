// SPDX-License-Identifier: AGPL-3.0-only
//! Each changed strict-sign Work pin is exercised at its exact and one-below limit.

use crate::testing::common::{TestResult, real};
use crate::testing::readings::{Fixture, ledger};
use crate::testing::strict_sign::{expected, fixture};
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, Handle};

/// Original operation selected by the independent integer oracle.
#[derive(Clone, Copy)]
pub(super) enum Subject {
    /// Product, quotient, reciprocal or negation in the original fixture sequence.
    Indexed(usize),
    /// An even or odd power built in a fresh fixture.
    Power(u32),
}

/// Reproduce the exact source state, including proofs preceding a sequential measurement.
fn prepared(signs: (i8, i8), subject: Subject, warm: bool) -> TestResult<(Fixture, Handle)> {
    let (mut source, subjects) = fixture(signs.0, signs.1)?;
    let handle = match subject {
        Subject::Indexed(index) => {
            if warm {
                for earlier in subjects.iter().take(index) {
                    source
                        .cas
                        .prove_strict_sign(&source.context, earlier, &mut ledger()?)?;
                }
            }
            subjects.get(index).ok_or("sign boundary subject")?.clone()
        }
        Subject::Power(exponent) => {
            let original = source.symbol("L")?;
            source.term(
                Operator::IntegerPower(i64::from(exponent)),
                real(),
                &[original],
            )?
        }
    };
    Ok((source, handle))
}

/// Refusal is specifically Work exhaustion, and successful consumption equals the pinned total.
fn outcome<T>(result: Result<T, Fault>, budget: &Budget, required: u64, short: u64) -> TestResult {
    if short == 0 {
        result?;
        assert_eq!(budget.usage().work, required);
    } else {
        assert!(
            matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: BudgetKind::Work, needed, .. }, .. }) if needed == required)
        );
    }
    Ok(())
}

/// Check production and fresh replay separately without changing a production ceiling.
pub(super) fn check(
    signs: (i8, i8),
    subject: Subject,
    positive: bool,
    pins: (u64, u64),
) -> TestResult {
    for short in [0, 1] {
        let (mut source, handle) = prepared(signs, subject, true)?;
        let mut limits = ledger()?.limits().clone();
        limits.work = pins.0 - short;
        let mut budget = Budget::new(limits)?;
        let result = source
            .cas
            .prove_strict_sign(&source.context, &handle, &mut budget);
        outcome(result, &budget, pins.0, short)?;
    }
    let (mut source, handle) = prepared(signs, subject, true)?;
    let witness = source
        .cas
        .prove_strict_sign(&source.context, &handle, &mut ledger()?)?;
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    for short in [0, 1] {
        let (mut receiver, handle) = prepared(signs, subject, false)?;
        let claim = expected(&receiver, &handle, positive)?;
        let mut limits = ledger()?.limits().clone();
        limits.work = pins.1 - short;
        let mut budget = Budget::new(limits)?;
        let before = receiver.cas.counts();
        let result = receiver.cas.replay(&bytes, &claim, &mut budget);
        if short != 0 {
            assert_eq!(receiver.cas.counts(), before);
        }
        outcome(result, &budget, pins.1, short)?;
    }
    Ok(())
}
