// SPDX-License-Identifier: AGPL-3.0-only
//! Three individually prepaid authentication visits on both frozen-store routes.
use super::*;

/// Exact byte handoff and every interrupted Work prefix preserve the caller ledger.
pub(super) fn boundaries(
    token: &LcOmegaDeclarationToken,
    certificate: Option<&nrz_cas::enclosure::lc_root::Certificate>,
) -> TestResult {
    let checked = run(token, certificate, &mut Budget::new(Limits::default())?)?;
    let retained = oracle::retained(certificate.is_some());
    for remaining in 0..=3 {
        let mut budget = Budget::new(Limits {
            work: PRIOR + remaining,
            ..Limits::default()
        })?;
        budget.work(PRIOR, AT)?;
        let allowance = budget.limits().staged_payload_bytes;
        let result = budget.with_retained_payload(CALLER + ORIGINAL - 424, AT, |budget| {
            checked.authenticate_join(token, 1, budget)
        });
        if remaining == 3 {
            result?;
        } else {
            exhausted(
                result,
                BudgetKind::Work,
                PRIOR + remaining,
                PRIOR + remaining + 1,
            )?;
        }
        assert_eq!(budget.usage().work, PRIOR + remaining);
        assert_eq!(budget.limits().staged_payload_bytes, allowance);
        assert_eq!(checked.retained_bytes(), retained);
    }
    let exact = CALLER + ORIGINAL + retained;
    for ceiling in [exact, exact - 1] {
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: ceiling,
            ..Limits::default()
        })?;
        budget.work(PRIOR, AT)?;
        let result = budget.with_retained_payload(CALLER + ORIGINAL - 424, AT, |budget| {
            checked.authenticate_join(token, 1, budget)
        });
        if ceiling == exact {
            result?;
            assert_eq!(budget.usage().work, PRIOR + 3);
        } else {
            exhausted(
                result,
                BudgetKind::StagedPayloadBytes,
                424 + retained - 1,
                424 + retained,
            )?;
            assert_eq!(budget.usage().work, PRIOR);
        }
        assert_eq!(budget.limits().staged_payload_bytes, ceiling);
        budget.check(BudgetKind::StagedPayloadBytes, ceiling, AT)?;
        assert_eq!(checked.retained_bytes(), retained);
    }
    Ok(())
}

/// Public mismatches and defensive foreign receipts preserve ownership and allow retry.
pub(super) fn failures(token: &LcOmegaDeclarationToken, preparation: &mut Budget) -> TestResult {
    let f = fixture(preparation)?;
    let foreign_token = owning::admit(f.request(), preparation)?;
    let mut foreign = run(&foreign_token, None, preparation)?;
    let certificate = run(token, None, preparation)?.certificate(preparation)?;
    for proposed in [None, Some(&certificate)] {
        let mut checked = run(token, proposed, preparation)?;
        let retained = checked.retained_bytes();
        let address = &raw const checked;
        for refusal in 0..3 {
            let mut budget = Budget::new(Limits::default())?;
            budget.work(PRIOR, AT)?;
            let allowance = budget.limits().staged_payload_bytes;
            // The independent foreign owner and correspondence stay caller-reserved.
            let caller = CALLER + ORIGINAL - 424
                + foreign_token.retained_bytes()?
                + foreign.retained_bytes();
            let result = budget.with_retained_payload(caller, AT, |budget| {
                match refusal {
                    0 => checked.authenticate_join(&foreign_token, 1, budget),
                    1 => checked.authenticate_join(token, 2, budget),
                    // Defensive only: the public API cannot replace this receipt.
                    _ => crate::lc_omega::correspondence::controls::foreign_join_receipt(
                        &mut checked,
                        &mut foreign,
                        token,
                        budget,
                    ),
                }
            });
            if refusal == 2 {
                let foreign_receipt = matches!(
                    result,
                    Err(AdmissionError::Algebra(nrz_cas::Fault {
                        kind: nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::ForeignHandle),
                        ..
                    }))
                );
                assert!(foreign_receipt);
            } else {
                let mismatch = matches!(result, Err(AdmissionError::ValueMismatch));
                assert!(mismatch);
            }
            let spent = [2, 3, 1][refusal];
            assert_eq!(budget.usage().work, PRIOR + spent);
            assert_eq!(budget.limits().staged_payload_bytes, allowance);
            assert_eq!(address, &raw const checked);
            assert_eq!(checked.retained_bytes(), retained);
            budget.with_retained_payload(caller, AT, |budget| {
                checked.authenticate_join(token, 1, budget)
            })?;
            assert_eq!(budget.usage().work, PRIOR + spent + 3);
            assert_eq!(budget.limits().staged_payload_bytes, allowance);
            foreign.check_declaration(&foreign_token, preparation)?;
        }
    }
    Ok(())
}
