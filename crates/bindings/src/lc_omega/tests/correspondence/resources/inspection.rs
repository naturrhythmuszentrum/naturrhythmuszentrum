// SPDX-License-Identifier: AGPL-3.0-only
//! Caller handoff for operations on an already retained correspondence.
use super::*;

/// Both routes preserve the same original baseline and reserve their own frozen residence.
pub(super) fn boundaries(
    token: &LcOmegaDeclarationToken,
    certificate: Option<&nrz_cas::enclosure::lc_root::Certificate>,
) -> TestResult {
    let checked = run(token, certificate, &mut Budget::new(Limits::default())?)?;
    let retained = oracle::retained(certificate.is_some());
    for export in [false, true] {
        // certificate::copy charges Q and checks Q+24; token identity charges one.
        let work = if export { 10_119 } else { 1 };
        let temporary = if export { 10_119 + 24 } else { 0 };
        let staged = CALLER + ORIGINAL + retained + temporary;
        for kind in [BudgetKind::Work, BudgetKind::StagedPayloadBytes] {
            let exact = if kind == BudgetKind::Work {
                PRIOR + work
            } else {
                staged
            };
            for ceiling in [exact, exact - 1] {
                let mut limits = Limits::default();
                if kind == BudgetKind::Work {
                    limits.work = ceiling;
                } else {
                    limits.staged_payload_bytes = ceiling;
                }
                let mut budget = Budget::new(limits)?;
                budget.work(PRIOR, AT)?;
                let allowance = budget.limits().staged_payload_bytes;
                // Only U-owner and unrelated P remain outside. The methods supply
                // the owner increment and full correspondence residence internally.
                let result = budget.with_retained_payload(CALLER + ORIGINAL - 424, AT, |budget| {
                    if export {
                        checked.certificate(budget).map(|value| {
                            assert_eq!(value.retained_bytes(), 10_119 + 24);
                        })
                    } else {
                        checked.check_declaration(token, budget)
                    }
                });
                if ceiling == exact {
                    result?;
                    assert_eq!(budget.usage().work, PRIOR + work);
                } else {
                    let needed = if kind == BudgetKind::Work {
                        exact
                    } else if export {
                        temporary
                    } else {
                        424 + retained
                    };
                    exhausted(result, kind, needed - 1, needed)?;
                    assert_eq!(budget.usage().work, PRIOR);
                }
                assert_eq!(budget.limits().staged_payload_bytes, allowance);
                budget.check(BudgetKind::StagedPayloadBytes, allowance, AT)?;
            }
        }
    }
    Ok(())
}
