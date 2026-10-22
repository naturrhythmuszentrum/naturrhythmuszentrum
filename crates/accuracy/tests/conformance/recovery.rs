// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`recovery_transition_contract`] | accuracy | Protocol mistakes do not consume work and an absent budget stays unrequested. |
//! | [`refinement_limit_and_last_attempt_success`] | accuracy | Initial attempts and refinements are counted separately; acceptance uses the full bound. |
//! | [`certified_radius_stagnation_window`] | accuracy | Stagnation uses the certified radius rather than any residual diagnostic. |
//! | [`recovery_resource_stops`] | accuracy | Attempt/work exhaustion refuses before execution and preserves earlier evidence. |
//! | [`recovery_arithmetic_stopping_evidence`] | accuracy | Failures stop without losing valid bounds or inventing a successful comparison. |

//! Recovery protocol, finite stopping conditions and retained numerical evidence.

use crate::testing::{Binary, TestResult};
use nrz_accuracy::{
    AccuracyFailure, Budget, BudgetDecision, Limits, Nonnegative, Promotion, RecoveryPolicy,
    RecoveryStatus, ResourceLimit, StopReason,
};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure};

/// Only a reserved attempt may complete; absence of a budget never reports success.
fn protocol<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut run = RecoveryPolicy::new(3, 2, Promotion::Never)?.start::<P>(None, Limits::new(4, 20));
    assert_eq!(run.evidence().status(), RecoveryStatus::Ready);
    assert_eq!(run.evidence().attempts(), 0);
    assert_eq!(run.evidence().precision(), P::PRECISION);
    assert_eq!(run.evidence().limits(), Limits::new(4, 20));
    assert!(matches!(
        run.complete(Nonnegative::ONE, &context),
        Err(AccuracyFailure::InvalidRecoveryState)
    ));
    assert_eq!(run.charge(1), Err(AccuracyFailure::InvalidRecoveryState));
    assert!(matches!(
        run.fail(ArithmeticFailure::NonFinite),
        Err(AccuracyFailure::InvalidRecoveryState)
    ));
    assert_eq!(run.begin_attempt(2)?, 1);
    assert_eq!(
        run.begin_attempt(1),
        Err(AccuracyFailure::InvalidRecoveryState)
    );
    run.charge(1)?;
    let evidence = run.complete(Nonnegative::ONE, &context)?;
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::NotRequested)
    );
    assert_eq!(
        evidence
            .assessment()
            .map(nrz_accuracy::BudgetAssessment::decision),
        Some(BudgetDecision::NotRequested)
    );
    assert_eq!(
        (evidence.attempts(), evidence.work(), evidence.refinements()),
        (1, 3, 0)
    );
    assert_eq!(evidence.best_upper(), Some(Nonnegative::ONE));
    assert_eq!(evidence.last_upper(), Some(Nonnegative::ONE));
    assert!(matches!(
        run.complete(Nonnegative::ZERO, &context),
        Err(AccuracyFailure::InvalidRecoveryState)
    ));
    assert_eq!(
        run.begin_attempt(0),
        Err(AccuracyFailure::InvalidRecoveryState)
    );
    Ok(())
}

/// Protocol mistakes do not consume work and an absent budget stays unrequested.
/// ´claim:accuracy:recovery-transition-contract´
/// ´test:integration:recovery-transition-contract´
#[test]
fn recovery_transition_contract() -> TestResult {
    protocol::<f32>()?;
    protocol::<f64>()
}

/// Zero refinement stops after the initial bound; success on the last attempt wins.
fn iteration_limits<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let budget = Budget::new(P::ONE, P::ZERO, P::ONE)?;
    let two = Nonnegative::new(P::from_u8(2))?;
    let mut disabled =
        RecoveryPolicy::new(0, 1, Promotion::Never)?.start(Some(budget), Limits::new(2, 10));
    disabled.begin_attempt(1)?;
    assert_eq!(
        disabled.complete(two, &context)?.status(),
        RecoveryStatus::Stopped(StopReason::RefinementLimit)
    );
    let mut bounded =
        RecoveryPolicy::new(1, 1, Promotion::Never)?.start(Some(budget), Limits::new(2, 10));
    assert_eq!(bounded.budget(), Some(&budget));
    bounded.begin_attempt(3)?;
    assert_eq!(
        bounded.complete(two, &context)?.status(),
        RecoveryStatus::Ready
    );
    assert_eq!(bounded.begin_attempt(4)?, 2);
    let evidence = bounded.complete(Nonnegative::ONE, &context)?;
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::BudgetMet)
    );
    assert_eq!(
        evidence
            .assessment()
            .map(nrz_accuracy::BudgetAssessment::decision),
        Some(BudgetDecision::Met)
    );
    assert_eq!(
        (evidence.attempts(), evidence.work(), evidence.refinements()),
        (2, 7, 1)
    );
    assert_eq!(evidence.best_upper(), Some(Nonnegative::ONE));
    Ok(())
}

/// Initial attempts and refinements are counted separately; acceptance uses the full bound.
/// ´claim:accuracy:refinement-limit-and-last-attempt-success´
/// ´test:integration:refinement-limit-and-last-attempt-success´
#[test]
fn refinement_limit_and_last_attempt_success() -> TestResult {
    iteration_limits::<f32>()?;
    iteration_limits::<f64>()
}

/// Rebounding to an old best is not progress; a strict improvement resets the window.
fn stagnation<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let budget = Budget::new(P::ONE, P::ZERO, P::ONE)?;
    let policy = RecoveryPolicy::new(10, 2, Promotion::Never)?;
    let mut stopped = policy.start(Some(budget), Limits::new(20, 100));
    for radius in [8, 6, 7, 6] {
        stopped.begin_attempt(1)?;
        stopped.complete(Nonnegative::new(P::from_u8(radius))?, &context)?;
    }
    assert_eq!(
        stopped.evidence().status(),
        RecoveryStatus::Stopped(StopReason::Stagnation)
    );
    assert_eq!(stopped.evidence().stagnant_attempts(), 2);
    assert_eq!(
        stopped.evidence().best_upper(),
        Some(Nonnegative::new(P::from_u8(6))?)
    );
    let mut improved = policy.start(Some(budget), Limits::new(20, 100));
    for radius in [8, 8, 7, 7, 6] {
        improved.begin_attempt(1)?;
        assert_eq!(
            improved
                .complete(Nonnegative::new(P::from_u8(radius))?, &context)?
                .status(),
            RecoveryStatus::Ready
        );
    }
    assert_eq!(improved.evidence().stagnant_attempts(), 0);
    improved.begin_attempt(1)?;
    assert_eq!(
        improved.complete(Nonnegative::ONE, &context)?.status(),
        RecoveryStatus::Stopped(StopReason::BudgetMet)
    );
    Ok(())
}

/// Stagnation uses the certified radius rather than any residual diagnostic.
/// ´claim:accuracy:certified-radius-stagnation-window´
/// ´test:integration:certified-radius-stagnation-window´
#[test]
fn certified_radius_stagnation_window() -> TestResult {
    stagnation::<f32>()?;
    stagnation::<f64>()
}

/// Resource refusals end the run with accurate counters and any earlier valid bound.
fn resource_stops<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let policy = RecoveryPolicy::new(10, 3, Promotion::Never)?;
    let budget = Budget::new(P::ZERO, P::ZERO, P::ONE)?;
    let mut limited = policy.start(Some(budget), Limits::new(1, 10));
    limited.begin_attempt(2)?;
    limited.complete(Nonnegative::ONE, &context)?;
    assert_eq!(
        limited.begin_attempt(1),
        Err(AccuracyFailure::ResourceLimit(ResourceLimit::Attempts))
    );
    let evidence = limited.evidence();
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::ResourceLimit(ResourceLimit::Attempts))
    );
    assert_eq!((evidence.attempts(), evidence.work()), (1, 2));
    assert_eq!(evidence.last_upper(), Some(Nonnegative::ONE));
    let mut exhausted = policy.start::<P>(None, Limits::new(3, 3));
    exhausted.begin_attempt(2)?;
    assert_eq!(
        exhausted.charge(2),
        Err(AccuracyFailure::ResourceLimit(ResourceLimit::Work))
    );
    assert_eq!(
        exhausted.evidence().status(),
        RecoveryStatus::Stopped(StopReason::ResourceLimit(ResourceLimit::Work))
    );
    assert_eq!(
        (exhausted.evidence().attempts(), exhausted.evidence().work()),
        (1, 2)
    );
    assert_eq!(exhausted.evidence().last_upper(), None);
    let mut empty = policy.start::<P>(None, Limits::new(0, 0));
    assert_eq!(
        empty.begin_attempt(0),
        Err(AccuracyFailure::ResourceLimit(ResourceLimit::Attempts))
    );
    assert_eq!(empty.evidence().attempts(), 0);
    Ok(())
}

/// Attempt/work exhaustion refuses before execution and preserves earlier evidence.
/// ´claim:accuracy:recovery-resource-stops´
/// ´test:integration:recovery-resource-stops´
#[test]
fn recovery_resource_stops() -> TestResult {
    resource_stops::<f32>()?;
    resource_stops::<f64>()
}

/// Nonfinite attempts retain earlier certificates; failed thresholds retain the new radius.
fn arithmetic_stops<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let policy = RecoveryPolicy::new(2, 2, Promotion::Never)?;
    let budget = Budget::new(P::ZERO, P::ZERO, P::ONE)?;
    let mut failed = policy.start(Some(budget), Limits::new(5, 100));
    failed.begin_attempt(1)?;
    failed.complete(Nonnegative::ONE, &context)?;
    failed.begin_attempt(2)?;
    let evidence = failed.fail(ArithmeticFailure::NonFinite)?;
    assert_eq!(
        evidence.status(),
        RecoveryStatus::Stopped(StopReason::Arithmetic(ArithmeticFailure::NonFinite))
    );
    assert_eq!(evidence.last_upper(), Some(Nonnegative::ONE));
    assert_eq!(
        evidence
            .assessment()
            .map(nrz_accuracy::BudgetAssessment::decision),
        Some(BudgetDecision::Exceeded)
    );
    assert_eq!((evidence.attempts(), evidence.work()), (2, 3));
    let overflow = Budget::new(P::ZERO, P::max_finite(), P::from_u8(2))?;
    let mut comparison = policy.start(Some(overflow), Limits::new(5, 100));
    comparison.begin_attempt(1)?;
    assert!(matches!(
        comparison.complete(Nonnegative::ONE, &context),
        Err(AccuracyFailure::Arithmetic(ArithmeticFailure::Overflow))
    ));
    assert_eq!(comparison.evidence().last_upper(), Some(Nonnegative::ONE));
    assert_eq!(comparison.evidence().best_upper(), Some(Nonnegative::ONE));
    assert_eq!(comparison.evidence().assessment(), None);
    assert_eq!(
        comparison.evidence().status(),
        RecoveryStatus::Stopped(StopReason::Arithmetic(ArithmeticFailure::Overflow))
    );
    Ok(())
}

/// Failures stop without losing valid bounds or inventing a successful comparison.
/// ´claim:accuracy:recovery-arithmetic-stopping-evidence´
/// ´test:integration:recovery-arithmetic-stopping-evidence´
#[test]
fn recovery_arithmetic_stopping_evidence() -> TestResult {
    arithmetic_stops::<f32>()?;
    arithmetic_stops::<f64>()
}
