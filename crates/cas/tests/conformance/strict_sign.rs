// SPDX-License-Identifier: AGPL-3.0-only
//! Strict-sign refusals preserve every premise and the caller's cumulative resources.

use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use crate::testing::strict_sign::{expected, fixture};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, InvalidCode, Location, Rational};

/// Missing, weak and zero denominator signs refuse before publishing any proof.
/// ´test:integration:strict-sign-unsigned-factors-refuse´
#[test]
fn strict_sign_unsigned_factors_refuse() -> TestResult {
    for signs in [(0, 1), (1, 0), (1, 2)] {
        let (mut source, subjects) = fixture(signs.0, signs.1)?;
        let before = source.cas.counts();
        for subject in subjects {
            let result = source
                .cas
                .prove_strict_sign(&source.context, &subject, &mut ledger()?);
            assert_eq!(
                result.err().map(|fault| fault.kind),
                Some(FaultKind::Invalid(InvalidCode::StrictSignUnavailable))
            );
            assert_eq!(source.cas.counts(), before);
        }
    }
    let (mut source, _) = fixture(1, 1)?;
    let zero = source.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let invalid = source.term(Operator::Div, real(), &[source.symbol("L")?, zero])?;
    assert!(
        source
            .cas
            .prove_strict_sign(&source.context, &invalid, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Removing either strict premise or changing its stratum cannot replay the original theorem.
/// ´test:integration:strict-sign-replay-retains-premises´
#[test]
fn strict_sign_replay_retains_premises() -> TestResult {
    let (mut source, subjects) = fixture(1, 1)?;
    let witness = source
        .cas
        .prove_strict_sign(&source.context, &subjects[3], &mut ledger()?)?;
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    for signs in [(0, 1), (1, 0), (-1, 1), (1, -1)] {
        let (mut receiver, originals) = fixture(signs.0, signs.1)?;
        let claim = expected(&receiver, &originals[3], false)?;
        let before = receiver.cas.counts();
        assert!(receiver.cas.replay(&bytes, &claim, &mut ledger()?).is_err());
        assert_eq!(receiver.cas.counts(), before);
    }
    Ok(())
}

fn run(
    limit: u64,
    kind: BudgetKind,
    prior: u64,
) -> TestResult<(Result<nrz_cas::proof::VerifiedWitness, Fault>, Budget)> {
    let (mut source, subjects) = fixture(1, 1)?;
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        BudgetKind::PayloadBytes => limits.payload_bytes = limit,
        BudgetKind::OutputBytes => limits.output_bytes = limit,
        _ => return Err("unsupported resource fixture".into()),
    }
    let mut budget = Budget::new(limits)?;
    budget.work(prior, Location::new("prior sign work"))?;
    let before = source.cas.counts();
    let result = source
        .cas
        .prove_strict_sign(&source.context, &subjects[3], &mut budget);
    if result.is_err() {
        assert_eq!(source.cas.counts(), before);
    }
    Ok((result, budget))
}

/// Proposal traversal, proof checking and encoding share one exact cumulative Work allowance.
/// ´test:integration:strict-sign-work-boundary´
#[test]
fn strict_sign_work_boundary() -> TestResult {
    let (result, full) = run(Limits::default().work, BudgetKind::Work, 0)?;
    result?;
    let cost = full.usage().work;
    assert_eq!(cost, 32_803);
    run(cost, BudgetKind::Work, 0)?.0?;
    let fault = run(cost - 1, BudgetKind::Work, 0)?
        .0
        .err()
        .ok_or("short Work accepted")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert!(run(cost, BudgetKind::Work, 1)?.0.is_err());
    Ok(())
}

/// Every new live proposal buffer and published witness has a tested exact payload boundary.
/// ´test:integration:strict-sign-payload-boundaries´
#[test]
fn strict_sign_payload_boundaries() -> TestResult {
    let mut measurements = Vec::new();
    for kind in [
        BudgetKind::StagedPayloadBytes,
        BudgetKind::PayloadBytes,
        BudgetKind::OutputBytes,
    ] {
        let mut low = 0;
        let mut high = Limits::default().staged_payload_bytes;
        run(high, kind, 0)?.0?;
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if run(middle, kind, 0)?.0.is_ok() {
                high = middle;
            } else {
                low = middle;
            }
        }
        measurements.push((kind, high));
        run(high, kind, 0)?.0?;
        let fault = run(high - 1, kind, 0)?
            .0
            .err()
            .ok_or("short payload accepted")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
    }
    assert_eq!(
        measurements,
        vec![
            (BudgetKind::StagedPayloadBytes, 24_032),
            (BudgetKind::PayloadBytes, 9_777),
            (BudgetKind::OutputBytes, 825),
        ]
    );
    Ok(())
}
