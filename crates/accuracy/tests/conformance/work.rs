// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`work_reservations_are_atomic`] | accuracy | An attempt charges both counters atomically and refusals preserve the old totals. |
//! | [`zero_and_integer_work_limits`] | accuracy | Zero allowances refuse work; maximum integer work never wraps to a new allowance. |
//! | [`recovery_policy_domain`] | accuracy | A positive window is required; zero refinements and explicit no-promotion are valid. |

//! Public cumulative work accounting at exact limits and integer boundaries.

use crate::testing::TestResult;
use nrz_accuracy::{AccuracyFailure, Limits, Promotion, RecoveryPolicy, ResourceLimit};

/// An attempt charges both counters atomically and refusals preserve the old totals.
/// ´claim:accuracy:work-reservations-are-atomic´
/// ´test:integration:work-reservations-are-atomic´
#[test]
fn work_reservations_are_atomic() -> TestResult {
    let limits = Limits::new(2, 10);
    assert_eq!(limits.max_attempts(), 2);
    assert_eq!(limits.max_work(), 10);
    let mut work = limits.start();
    assert_eq!(work.limits(), limits);
    assert_eq!(work.reserve_attempt(4)?, 1);
    assert_eq!(work.reserve_attempt(7), Err(ResourceLimit::Work));
    assert_eq!((work.attempts(), work.work()), (1, 4));
    work.charge(2)?;
    assert_eq!(work.reserve_attempt(4)?, 2);
    assert_eq!((work.attempts(), work.work()), (2, 10));
    assert_eq!(work.reserve_attempt(0), Err(ResourceLimit::Attempts));
    assert_eq!(work.charge(1), Err(ResourceLimit::Work));
    work.charge(0)?;
    assert_eq!((work.attempts(), work.work()), (2, 10));
    Ok(())
}

/// Zero allowances refuse work; maximum integer work never wraps to a new allowance.
/// ´claim:accuracy:zero-and-integer-work-limits´
/// ´test:integration:zero-and-integer-work-limits´
#[test]
fn zero_and_integer_work_limits() -> TestResult {
    let mut none = Limits::new(0, 0).start();
    assert_eq!(none.reserve_attempt(0), Err(ResourceLimit::Attempts));
    assert_eq!(none.charge(1), Err(ResourceLimit::Work));
    assert_eq!((none.attempts(), none.work()), (0, 0));
    let mut maximal = Limits::new(u32::MAX, u64::MAX).start();
    assert_eq!(maximal.reserve_attempt(u64::MAX)?, 1);
    assert_eq!(maximal.charge(1), Err(ResourceLimit::Work));
    assert_eq!(maximal.reserve_attempt(1), Err(ResourceLimit::Work));
    assert_eq!((maximal.attempts(), maximal.work()), (1, u64::MAX));
    let mut no_work = Limits::new(1, 0).start();
    assert_eq!(no_work.reserve_attempt(1), Err(ResourceLimit::Work));
    assert_eq!(no_work.reserve_attempt(0)?, 1);
    Ok(())
}

/// A positive window is required; zero refinements and explicit no-promotion are valid.
/// ´claim:accuracy:recovery-policy-domain´
/// ´test:integration:recovery-policy-domain´
#[test]
fn recovery_policy_domain() -> TestResult {
    assert_eq!(
        RecoveryPolicy::new(1, 0, Promotion::Never),
        Err(AccuracyFailure::InvalidPolicy)
    );
    let policy = RecoveryPolicy::new(0, 1, Promotion::Never)?;
    assert_eq!(policy.max_refinements(), 0);
    assert_eq!(policy.stagnation_window(), 1);
    assert_eq!(policy.promotion(), Promotion::Never);
    Ok(())
}
