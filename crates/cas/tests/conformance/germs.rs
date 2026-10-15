// SPDX-License-Identifier: AGPL-3.0-only
//! Punctured approach, gluing, mode, budget and deterministic witness contracts.

use crate::testing::boundary::{named, quadratic};
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use crate::testing::germs::{at, task};
use nrz_cas::FaultKind;
use nrz_cas::domain::Side;
use nrz_cas::limits::BudgetKind;
use nrz_cas::proof::GermMode;
use nrz_cas::request::{Outcome, RefusalReason};
use nrz_cas::syntax::{CaseName, PredicateKind};

/// The two sign traces disagree, so the requested two-sided limit is explicitly refused.
/// ´test:integration:case-limit-disagreement-refusal´
#[test]
fn case_limit_disagreement_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = named(&mut fixture, CaseName::Sign)?;
    let approach = at(&mut fixture, "0", None)?;
    let request = fixture.request(task(&approach, GermMode::Limit), &[function])?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Refused {
            reason: RefusalReason::AnalyticDisagreement,
            ..
        }
    ));
    Ok(())
}

/// An owned singleton has no punctured side neighborhood and cannot establish a limit.
/// ´test:integration:case-limit-empty-approach-pending´
#[test]
fn case_limit_empty_approach_pending() -> TestResult {
    let mut fixture = Fixture::new()?;
    let equal = fixture.predicate(
        PredicateKind::Equal,
        &[fixture.x.clone(), fixture.zero.clone()],
    )?;
    let region = fixture.region(&equal, true)?;
    let function = fixture.cases(&[(region, fixture.one.clone())])?;
    let approach = at(&mut fixture, "0", Some(Side::Above))?;
    let request = fixture.request(task(&approach, GermMode::Limit), &[function])?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::PendingPremise { .. }
    ));
    Ok(())
}

/// Matching first jets justify order-one Taylor data but cannot prove common quadratic or analytic germs.
/// ´test:integration:case-series-gluing-and-remainder-contracts´
#[test]
fn case_series_gluing_and_remainder_contracts() -> TestResult {
    for (order, analytic, succeeds) in [(1, false, true), (2, false, false), (1, true, false)] {
        let mut fixture = Fixture::new()?;
        let function = quadratic(&mut fixture, false)?;
        let approach = at(&mut fixture, "0", None)?;
        let request = fixture.request(
            task(&approach, GermMode::Series { order, analytic }),
            &[function],
        )?;
        let outcome = fixture.cas.run(&request);
        if succeeds {
            assert!(matches!(outcome, Outcome::Proved { .. }));
        } else {
            assert!(matches!(
                outcome,
                Outcome::Refused {
                    reason: RefusalReason::AnalyticDisagreement,
                    ..
                }
            ));
        }
    }
    let mut fixture = Fixture::new()?;
    let function = named(&mut fixture, CaseName::Ramp)?;
    let approach = at(&mut fixture, "0", Some(Side::Above))?;
    let request = fixture.request(
        task(
            &approach,
            GermMode::Series {
                order: 2,
                analytic: true,
            },
        ),
        &[function],
    )?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Refused {
            reason: RefusalReason::AnalyticDisagreement,
            ..
        }
    ));
    Ok(())
}

/// One three-entry jet requires six entries across independent checking and fresh replay.
/// ´test:integration:case-series-exact-jet-budgets´
#[test]
fn case_series_exact_jet_budgets() -> TestResult {
    for (kind, exact) in [(BudgetKind::JetOrder, 2), (BudgetKind::JetEntries, 6)] {
        for cap in [exact, exact - 1] {
            let mut fixture = Fixture::new()?;
            let function = named(&mut fixture, CaseName::Ramp)?;
            let approach = at(&mut fixture, "0", Some(Side::Above))?;
            let mut request = fixture.request(
                task(
                    &approach,
                    GermMode::Series {
                        order: 2,
                        analytic: false,
                    },
                ),
                &[function],
            )?;
            if kind == BudgetKind::JetOrder {
                request.limits.jet_order = Some(cap);
            } else {
                request.limits.jet_entries = Some(cap);
            }
            let outcome = fixture.cas.run(&request);
            if cap == exact {
                assert!(matches!(outcome, Outcome::Proved { .. }));
            } else {
                assert!(
                    matches!(outcome, Outcome::Exhausted { fault, .. } if matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind))
                );
            }
        }
    }
    Ok(())
}

/// Different allocation orders yield identical complete germ witnesses.
/// ´test:integration:case-germ-deterministic-bytes´
#[test]
fn case_germ_deterministic_bytes() -> TestResult {
    for mode in [
        GermMode::Limit,
        GermMode::Series {
            order: 2,
            analytic: false,
        },
    ] {
        let mut outputs = Vec::new();
        for extra in [false, true] {
            let mut fixture = Fixture::new()?;
            if extra {
                fixture.number("42")?;
            }
            let function = named(&mut fixture, CaseName::Ramp)?;
            let approach = at(&mut fixture, "0", Some(Side::Above))?;
            let request = fixture.request(task(&approach, mode), &[function])?;
            let result = fixture.proved(&request)?;
            outputs.push(
                fixture
                    .cas
                    .export_witness(&result.witness, &mut budget()?)?,
            );
        }
        assert_eq!(outputs.first(), outputs.get(1));
    }
    Ok(())
}
