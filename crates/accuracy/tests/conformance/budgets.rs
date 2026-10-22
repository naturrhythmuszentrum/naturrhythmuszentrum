// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`budget_decisions_rational_oracle`] | accuracy | Both formats contain exact tolerances and every decisive result has the right direction. |
//! | [`budget_boundary_decisions`] | accuracy | Equality, below/above thresholds, unresolved overlap and absence stay distinct. |
//! | [`tolerance_never_rounds_up_for_acceptance`] | accuracy | Rounded tolerance products and underflow never create false budget success. |
//! | [`budget_domain_and_overflow_refusals`] | accuracy | Invalid coefficients, zero/uncertain scales and tolerance overflow are explicit refusals. |

//! Independent absolute, relative and ambiguous tolerance comparisons.

use crate::testing::{Binary, TestResult};
use nrz_accuracy::{AccuracyFailure, Budget, BudgetDecision, Nonnegative, assess_budget};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite};

/// Compare successful decisions against independently decoded exact tolerance.
fn rational_decisions<P: Binary>(absolute: P, relative: P, scale: P, errors: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let budget = Budget::new(absolute, relative, scale)?;
    let exact = absolute.rational() + relative.rational() * scale.rational();
    let threshold = budget.tolerance(&context)?;
    assert!(threshold.lower().get().rational() <= exact);
    assert!(threshold.upper().get().rational() >= exact);
    assert_eq!(
        budget.absolute(),
        Enclosure::singleton(Finite::new(absolute)?)
    );
    assert_eq!(
        budget.relative(),
        Enclosure::singleton(Finite::new(relative)?)
    );
    assert_eq!(
        budget.reference_scale(),
        Enclosure::singleton(Finite::new(scale)?)
    );
    for &error in errors {
        let upper = Nonnegative::new(error)?;
        let result = assess_budget(Some(&budget), upper, &context)?;
        assert_eq!(result.upper(), upper);
        assert_eq!(result.tolerance(), Some(threshold));
        match result.decision() {
            BudgetDecision::Met => assert!(error.rational() <= exact),
            BudgetDecision::Exceeded => assert!(error.rational() > exact),
            BudgetDecision::Unresolved => {
                assert!(upper.finite() > threshold.lower());
                assert!(upper.finite() <= threshold.upper());
            }
            BudgetDecision::NotRequested => assert_eq!(result.decision(), BudgetDecision::Met),
        }
    }
    Ok(())
}

/// Both formats contain exact tolerances and every decisive result has the right direction.
/// ´claim:accuracy:budget-decisions-rational-oracle´
/// ´test:integration:budget-decisions-rational-oracle´
#[test]
fn budget_decisions_rational_oracle() -> TestResult {
    rational_decisions(1.0_f32, 0.0, 2.0, &[0.0, 0.5, 1.0, 2.0])?;
    rational_decisions(1.0_f64, 0.0, 2.0, &[0.0, 0.5, 1.0, 2.0])?;
    rational_decisions(0.0_f32, 1.0, 2.0, &[1.0, 2.0, 3.0])?;
    rational_decisions(0.0_f64, 1.0, 2.0, &[1.0, 2.0, 3.0])?;
    rational_decisions(0.1_f32, 0.2, 10.0, &[0.0, 1.0, 2.1, 3.0])?;
    rational_decisions(0.1_f64, 0.2, 10.0, &[0.0, 1.0, 2.1, 3.0])?;
    rational_decisions(0.0_f32, f32::from_bits(1), 0.5, &[0.0, f32::from_bits(1)])?;
    rational_decisions(0.0_f64, f64::from_bits(1), 0.5, &[0.0, f64::from_bits(1)])
}

/// Exact absolute/relative equality succeeds while a supplied ambiguous threshold does not.
fn boundaries<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let two = P::from_u8(2);
    let three = P::from_u8(3);
    for request in [
        Budget::new(two, P::ZERO, P::ONE)?,
        Budget::new(P::ZERO, P::ONE, two)?,
    ] {
        for (error, expected) in [
            (P::ONE, BudgetDecision::Met),
            (two, BudgetDecision::Met),
            (three, BudgetDecision::Exceeded),
        ] {
            assert_eq!(
                assess_budget(Some(&request), Nonnegative::new(error)?, &context)?.decision(),
                expected
            );
        }
    }
    let ambiguous = Budget::from_enclosures(
        Enclosure::new(P::ONE, two)?,
        Enclosure::singleton(Finite::ZERO),
        Enclosure::singleton(Finite::ONE),
    )?;
    for (error, expected) in [
        (P::ZERO, BudgetDecision::Met),
        (P::ONE, BudgetDecision::Met),
        (P::from_u8(3) / two, BudgetDecision::Unresolved),
        (two, BudgetDecision::Unresolved),
        (three, BudgetDecision::Exceeded),
    ] {
        assert_eq!(
            assess_budget(Some(&ambiguous), Nonnegative::new(error)?, &context)?.decision(),
            expected
        );
    }
    let zero = Budget::new(P::ZERO, P::ZERO, P::ONE)?;
    assert_eq!(
        assess_budget(Some(&zero), Nonnegative::ZERO, &context)?.decision(),
        BudgetDecision::Met
    );
    let absent = assess_budget::<P>(None, Nonnegative::ONE, &context)?;
    assert_eq!(absent.decision(), BudgetDecision::NotRequested);
    assert_eq!(absent.tolerance(), None);
    Ok(())
}

/// Equality, below/above thresholds, unresolved overlap and absence stay distinct.
/// ´claim:accuracy:budget-boundary-decisions´
/// ´test:integration:budget-boundary-decisions´
#[test]
fn budget_boundary_decisions() -> TestResult {
    boundaries::<f32>()?;
    boundaries::<f64>()
}

/// A naively rounded-up product would accept an error larger than the exact tolerance.
fn rounded_up<P: Binary>(tenth: P) -> TestResult {
    let context = ArithmeticContext::new()?;
    let budget = Budget::new(P::ZERO, tenth, tenth)?;
    let naive = tenth * tenth;
    let exact = tenth.rational() * tenth.rational();
    assert!(naive.rational() > exact);
    assert_eq!(
        assess_budget(Some(&budget), Nonnegative::new(naive)?, &context)?.decision(),
        BudgetDecision::Unresolved
    );
    let underflow = Budget::new(P::ZERO, P::ZERO.next_up(), P::ONE / P::from_u8(2))?;
    let threshold = underflow.tolerance(&context)?;
    assert_eq!(threshold.lower(), Finite::ZERO);
    assert_eq!(
        assess_budget(
            Some(&underflow),
            Nonnegative::new(P::ZERO.next_up())?,
            &context
        )?
        .decision(),
        BudgetDecision::Unresolved
    );
    Ok(())
}

/// Rounded tolerance products and underflow never create false budget success.
/// ´claim:accuracy:tolerance-never-rounds-up-for-acceptance´
/// ´test:integration:tolerance-never-rounds-up-for-acceptance´
#[test]
fn tolerance_never_rounds_up_for_acceptance() -> TestResult {
    rounded_up(0.1_f32)?;
    rounded_up(0.1_f64)
}

/// Every coefficient and the whole scale enclosure must satisfy its stated domain.
fn invalid<P: Binary>() -> TestResult {
    for (absolute, relative, scale) in [
        (-P::ONE, P::ZERO, P::ONE),
        (P::ZERO, -P::ONE, P::ONE),
        (P::ZERO, P::ZERO, P::ZERO),
        (P::ONE, P::ZERO, -P::ONE),
    ] {
        assert_eq!(
            Budget::new(absolute, relative, scale),
            Err(AccuracyFailure::InvalidBudget)
        );
    }
    let crossing = Enclosure::new(-P::ONE, P::ONE)?;
    let zero = Enclosure::singleton(Finite::ZERO);
    let one = Enclosure::singleton(Finite::ONE);
    assert_eq!(
        Budget::from_enclosures(crossing, zero, one),
        Err(AccuracyFailure::InvalidBudget)
    );
    assert_eq!(
        Budget::from_enclosures(zero, crossing, one),
        Err(AccuracyFailure::InvalidBudget)
    );
    assert_eq!(
        Budget::from_enclosures(zero, zero, crossing),
        Err(AccuracyFailure::InvalidBudget)
    );
    assert_eq!(
        Budget::from_enclosures(zero, zero, Enclosure::new(P::ZERO, P::ONE)?),
        Err(AccuracyFailure::InvalidBudget)
    );
    let context = ArithmeticContext::new()?;
    let overflowing = Budget::new(P::ZERO, P::max_finite(), P::from_u8(2))?;
    assert_eq!(
        assess_budget(Some(&overflowing), Nonnegative::ONE, &context),
        Err(ArithmeticFailure::Overflow)
    );
    Ok(())
}

/// Invalid coefficients, zero/uncertain scales and tolerance overflow are explicit refusals.
/// ´claim:accuracy:budget-domain-and-overflow-refusals´
/// ´test:integration:budget-domain-and-overflow-refusals´
#[test]
fn budget_domain_and_overflow_refusals() -> TestResult {
    invalid::<f32>()?;
    invalid::<f64>()?;
    for invalid_value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            Budget::new(invalid_value, 0.0, 1.0),
            Err(AccuracyFailure::InvalidBudget)
        );
        assert_eq!(
            Budget::new(0.0, invalid_value, 1.0),
            Err(AccuracyFailure::InvalidBudget)
        );
        assert_eq!(
            Budget::new(0.0, 0.0, invalid_value),
            Err(AccuracyFailure::InvalidBudget)
        );
    }
    for invalid_value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Budget::new(invalid_value, 0.0, 1.0),
            Err(AccuracyFailure::InvalidBudget)
        );
        assert_eq!(
            Budget::new(0.0, invalid_value, 1.0),
            Err(AccuracyFailure::InvalidBudget)
        );
        assert_eq!(
            Budget::new(0.0, 0.0, invalid_value),
            Err(AccuracyFailure::InvalidBudget)
        );
    }
    Ok(())
}
