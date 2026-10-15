// SPDX-License-Identifier: AGPL-3.0-only
//! Public canonical fraction admission, sharing, resource and atomicity contracts.

use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::Fixture;
use nrz_cas::FaultKind;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::solving::{CoefficientValueData, SolveView};

/// Coprime x and y are admitted without asserting a multivariate Bezout-to-one identity.
/// ´test:integration:fraction-coprime-variables´
#[test]
fn fraction_coprime_variables() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let values = fixture.cas.admit(
        std::slice::from_ref(&draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let value = values.first().ok_or("missing fraction")?;
    let SolveView::CoefficientValue(data) = fixture.cas.solve_view(value)? else {
        return Err("wrong fraction view".into());
    };
    assert_eq!(
        data.value,
        CoefficientValueData::Fraction {
            numerator,
            denominator
        }
    );
    let before = fixture.cas.counts();
    let shared = fixture.cas.admit(
        std::slice::from_ref(&draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(shared, values);
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// A common polynomial factor and a nonmonic denominator independently invalidate admission.
/// ´test:integration:fraction-normalization-mutations´
#[test]
fn fraction_normalization_mutations() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![2, 0], 1), (vec![0, 0], -1)])?;
    let common = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], -1)])?;
    let scaled = fixture.polynomial(2, &[(vec![0, 1], 2), (vec![0, 0], 2)])?;
    for denominator in [&common, &scaled] {
        let draft = fixture.fraction(&numerator, denominator);
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .admit(
                    std::slice::from_ref(&draft),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Canonical gcd admission succeeds at its measured operation count and refuses at one fewer atomically.
/// ´test:integration:fraction-gcd-budget-boundary´
#[test]
fn fraction_gcd_budget_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture
        .cas
        .admit(std::slice::from_ref(&draft), &mut measured)?;
    let count = measured.usage().gcd_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let mut peer = Fixture::new()?;
        let top = peer.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], 1)])?;
        let bottom = peer.polynomial(2, &[(vec![0, 1], 1)])?;
        let proposed = peer.fraction(&top, &bottom);
        let before = peer.cas.counts();
        let mut budget = Budget::new(Limits {
            gcd_steps: limit,
            ..Limits::blind_cubic()
        })?;
        let result = peer.cas.admit(std::slice::from_ref(&proposed), &mut budget);
        if succeeds {
            assert!(result.is_ok());
        } else {
            let failure = result.err().ok_or("short gcd budget accepted")?;
            assert!(matches!(
                failure.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::GcdSteps,
                    ..
                }
            ));
            assert_eq!(peer.cas.counts(), before);
        }
    }
    Ok(())
}
