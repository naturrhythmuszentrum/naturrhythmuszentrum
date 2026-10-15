// SPDX-License-Identifier: AGPL-3.0-only
//! Bound vectors through fixed budgets, bounded refinement and explicit precision retry.

use crate::testing::{Binary, Rational, TestResult};
use nrz_accuracy::{
    AccuracyFailure, BoundVectorRef, Budget, BudgetDecision, Limits, Nonnegative, NormKind,
    Promotion, RecoveryPolicy, RecoveryStatus, ResourceLimit, StopReason,
};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure};

/// Each attempt independently summarizes all entries before the policy sees its radius.
fn vector_policy<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let policy = RecoveryPolicy::new(2, 2, Promotion::Never)?;
    let request = Budget::new(P::ONE, P::ZERO, P::ONE)?;
    let mut run = policy.start(Some(request), Limits::new(3, 30));
    let vectors = [
        [P::from_u8(3), P::from_u8(4)],
        [P::ONE, P::ONE],
        [P::ONE / P::from_u8(4), P::ONE / P::from_u8(2)],
    ];
    for entries in vectors {
        run.begin_attempt(10)?;
        let summary = BoundVectorRef::new(&entries)?.norm(NormKind::Euclidean, &context)?;
        let squares: Rational = entries
            .iter()
            .map(|value| {
                let exact = value.rational();
                &exact * &exact
            })
            .sum();
        let upper = summary.upper().get().rational();
        assert!(&upper * &upper >= squares);
        let evidence = run.complete(summary.upper(), &context)?;
        if evidence
            .assessment()
            .is_some_and(|assessment| assessment.decision() == BudgetDecision::Met)
        {
            assert!(squares <= P::ONE.rational());
        }
    }
    let evidence = run.evidence();
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::BudgetMet)
    );
    assert_eq!(
        (evidence.attempts(), evidence.work(), evidence.refinements()),
        (3, 30, 2)
    );
    assert_eq!(evidence.stagnant_attempts(), 0);
    Ok(())
}

/// Several independent vectors flow through a bounded policy without a physical assurance claim.
/// ´test:integration:bound-vector-policy-lifecycle´
#[test]
fn bound_vector_policy_lifecycle() -> TestResult {
    vector_policy::<f32>()?;
    vector_policy::<f64>()
}

/// Widening preserves every budget endpoint and transfers cumulative work, not old success.
/// ´test:integration:precision-retry-retains-work-and-budget-width´
#[test]
fn precision_retry_retains_work_and_budget_width() -> TestResult {
    let context = ArithmeticContext::new()?;
    let budget = Budget::from_enclosures(
        Enclosure::new(0.25_f32, 0.5)?,
        Enclosure::new(0.1, 0.2)?,
        Enclosure::new(2.0, 3.0)?,
    )?;
    let policy = RecoveryPolicy::new(0, 1, Promotion::F32ToF64)?;
    let mut first = policy.start(Some(budget), Limits::new(2, 8));
    first.begin_attempt(3)?;
    first.complete(Nonnegative::new(10.0)?, &context)?;
    assert!(first.can_retry_f64());
    let (mut promoted, earlier) = first.retry_f64()?;
    assert_eq!(
        earlier.status(),
        RecoveryStatus::Stopped(StopReason::RefinementLimit)
    );
    assert_eq!(earlier.last_upper(), Some(Nonnegative::new(10.0)?));
    assert_eq!(promoted.evidence().last_upper(), None);
    assert_eq!(promoted.evidence().best_upper(), None);
    assert_eq!(promoted.evidence().assessment(), None);
    assert_eq!(promoted.evidence().refinements(), 0);
    assert_eq!(promoted.budget().copied(), Some(budget.widen()));
    let wider = budget.widen();
    for (old, new) in [
        (budget.absolute(), wider.absolute()),
        (budget.relative(), wider.relative()),
        (budget.reference_scale(), wider.reference_scale()),
    ] {
        assert_eq!(old.lower().get().rational(), new.lower().get().rational());
        assert_eq!(old.upper().get().rational(), new.upper().get().rational());
    }
    assert_eq!(promoted.begin_attempt(4)?, 2);
    promoted.charge(1)?;
    let evidence = promoted.complete(Nonnegative::ZERO, &context)?;
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::BudgetMet)
    );
    assert_eq!((evidence.attempts(), evidence.work()), (2, 8));
    Ok(())
}

/// An authorized retry does not reset exhausted resources, even after nonfinite arithmetic.
/// ´test:integration:precision-retry-cannot-reset-limits´
#[test]
fn precision_retry_cannot_reset_limits() -> TestResult {
    let policy = RecoveryPolicy::new(0, 1, Promotion::F32ToF64)?;
    for cause in [ArithmeticFailure::NonFinite, ArithmeticFailure::Overflow] {
        let mut first = policy.start::<f32>(None, Limits::new(1, 3));
        first.begin_attempt(3)?;
        first.fail(cause)?;
        assert!(first.can_retry_f64());
        let (mut promoted, earlier) = first.retry_f64()?;
        assert_eq!(earlier.last_upper(), None);
        assert_eq!(
            promoted.begin_attempt(0),
            Err(AccuracyFailure::ResourceLimit(ResourceLimit::Attempts))
        );
        assert_eq!(
            (promoted.evidence().attempts(), promoted.evidence().work()),
            (1, 3)
        );
    }
    let mut first = policy.start::<f32>(None, Limits::new(2, 3));
    first.begin_attempt(3)?;
    first.fail(ArithmeticFailure::NonFinite)?;
    let (mut promoted, _) = first.retry_f64()?;
    assert_eq!(
        promoted.begin_attempt(1),
        Err(AccuracyFailure::ResourceLimit(ResourceLimit::Work))
    );
    assert_eq!(
        (promoted.evidence().attempts(), promoted.evidence().work()),
        (1, 3)
    );
    Ok(())
}

/// Success, lack of a request, forbidden promotion and unsupported arithmetic do not authorize retry.
/// ´test:integration:precision-retry-permission-boundaries´
#[test]
fn precision_retry_permission_boundaries() -> TestResult {
    let context = ArithmeticContext::new()?;
    let allowed = RecoveryPolicy::new(0, 1, Promotion::F32ToF64)?;
    let mut active = allowed.start::<f32>(None, Limits::new(3, 10));
    assert!(!active.can_retry_f64());
    active.begin_attempt(1)?;
    assert!(!active.can_retry_f64());
    active.complete(Nonnegative::ONE, &context)?;
    assert!(!active.can_retry_f64());
    assert!(matches!(
        active.retry_f64(),
        Err(AccuracyFailure::InvalidRecoveryState)
    ));
    let request = Budget::new(1.0_f32, 0.0, 1.0)?;
    let mut met = allowed.start(Some(request), Limits::new(3, 10));
    met.begin_attempt(1)?;
    met.complete(Nonnegative::ONE, &context)?;
    assert!(!met.can_retry_f64());
    let mut refused =
        RecoveryPolicy::new(0, 1, Promotion::Never)?.start::<f32>(None, Limits::new(3, 10));
    refused.begin_attempt(1)?;
    refused.fail(ArithmeticFailure::NonFinite)?;
    assert!(matches!(
        refused.retry_f64(),
        Err(AccuracyFailure::InvalidRecoveryState)
    ));
    let mut environment = allowed.start::<f32>(None, Limits::new(3, 10));
    environment.begin_attempt(1)?;
    environment.fail(ArithmeticFailure::UnsupportedEnvironment)?;
    assert!(!environment.can_retry_f64());
    let mut exhausted = allowed.start::<f32>(None, Limits::new(0, 0));
    assert_eq!(
        exhausted.begin_attempt(0),
        Err(AccuracyFailure::ResourceLimit(ResourceLimit::Attempts))
    );
    assert!(!exhausted.can_retry_f64());
    Ok(())
}
