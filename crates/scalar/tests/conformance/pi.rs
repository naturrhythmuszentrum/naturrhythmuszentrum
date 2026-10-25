// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational pi witnesses and transactional resource refusals.

use crate::testing::{Binary, Rational};
use nrz_scalar::{
    ArithmeticContext, Finite, PiBounds, PiBudget, PiEnclosure, PiError, PiLimits, PiUsage,
};
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const fn budget() -> PiBudget {
    PiBudget::new(PiLimits {
        work: 100_000,
        steps: 100,
        staged_bytes: 1024,
        output_bytes: 4096,
    })
}

/// An independent identity: `pi/4 = atan(1/2) + atan(1/3)`; no Machin coefficients.
fn independent_bounds() -> (Rational, Rational) {
    let (first_lower, first_upper) = arctangent(2);
    let (second_lower, second_upper) = arctangent(3);
    (
        (first_lower + second_lower) * BigInt::from(4),
        (first_upper + second_upper) * BigInt::from(4),
    )
}

/// Eighty exact terms end with a negative term; the next positive term bounds the tail.
fn arctangent(divisor: u32) -> (Rational, Rational) {
    let mut sum = Rational::zero();
    let mut power = BigInt::from(divisor);
    for index in 0_u32..80 {
        let term = Rational::new(BigInt::one(), &power * (2 * index + 1));
        if index.is_multiple_of(2) {
            sum += term;
        } else {
            sum -= term;
        }
        power *= divisor * divisor;
    }
    let upper = &sum + Rational::new(BigInt::one(), power * 161);
    (sum, upper)
}

fn check<P: Binary>(result: PiEnclosure<P>, tolerance: P) {
    let (truth_lower, truth_upper) = independent_bounds();
    let exact = result.rational_bounds();
    let denominator = BigInt::one() << PiBounds::denominator_power();
    let lower = Rational::new(BigInt::from(exact.lower_numerator()), denominator.clone());
    let upper = Rational::new(BigInt::from(exact.upper_numerator()), denominator);
    let lower_contains = lower <= truth_lower;
    let upper_contains = truth_upper <= upper;
    assert!(lower_contains && upper_contains);
    assert!(result.enclosure().lower().get().rational() <= lower);
    assert!(result.enclosure().upper().get().rational() >= upper);
    let center = result.center().get().rational();
    let distance = (center.clone() - truth_lower)
        .abs()
        .max((center - truth_upper).abs());
    let bound = result.displacement().get().rational();
    let contains_distance = distance <= bound;
    let meets_tolerance = bound <= tolerance.rational();
    assert!(contains_distance && meets_tolerance);
    assert_eq!(result.revision(), 1);
}

/// Every refinement and format encloses pi by an independent exact rational theorem.
/// ´claim:scalar:pi-independent-rational-bounds´
/// ´test:integration:pi-independent-rational-bounds´
#[test]
fn pi_independent_rational_bounds() -> TestResult {
    let context = ArithmeticContext::new()?;
    for tolerance in [2.0_f32, 0.01, 0.000_001, 0.000_000_3] {
        let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut budget())?;
        check(result, tolerance);
    }
    for tolerance in [2.0_f64, 0.01, 1e-12, 1e-14, 5e-16] {
        let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut budget())?;
        check(result, tolerance);
    }
    Ok(())
}

/// Tightened requests retain nested rational evidence and include binary quantization.
/// ´claim:scalar:pi-refinement-is-nested´
/// ´test:integration:pi-refinement-is-nested´
#[test]
fn pi_refinement_is_nested() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = budget();
    let mut previous = PiEnclosure::new(Finite::new(2.0_f64)?, &context, &mut ledger)?;
    for tolerance in [0.1_f64, 0.001, 1e-8, 1e-12, 5e-16] {
        let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger)?;
        assert!(
            result.rational_bounds().lower_numerator()
                >= previous.rational_bounds().lower_numerator()
        );
        assert!(
            result.rational_bounds().upper_numerator()
                <= previous.rational_bounds().upper_numerator()
        );
        assert!(result.steps_used() >= previous.steps_used());
        previous = result;
    }
    Ok(())
}

/// Invalid tolerances and exhausted fixed resolution preserve nonzero caller usage.
/// ´claim:scalar:pi-tolerance-refusals-are-atomic´
/// ´test:integration:pi-tolerance-refusals-are-atomic´
#[test]
fn pi_tolerance_refusals_are_atomic() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = budget();
    PiEnclosure::new(Finite::new(2.0_f64)?, &context, &mut ledger)?;
    for (tolerance, refusal) in [
        (0.0_f64, PiError::InvalidTolerance),
        (-1.0, PiError::InvalidTolerance),
        (f64::MIN_POSITIVE, PiError::ToleranceUnreachable),
    ] {
        let before = ledger.usage();
        assert_eq!(
            PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger),
            Err(refusal)
        );
        assert_eq!(ledger.usage(), before);
    }
    let before = ledger.usage();
    assert_eq!(
        PiEnclosure::new(Finite::new(f32::MIN_POSITIVE)?, &context, &mut ledger),
        Err(PiError::ToleranceUnreachable)
    );
    assert_eq!(ledger.usage(), before);
    Ok(())
}

/// Independent caps refuse before publication even with a zero allowance.
/// ´claim:scalar:pi-zero-resource-refusals´
/// ´test:integration:pi-zero-resource-refusals´
#[test]
fn pi_zero_resource_refusals() -> TestResult {
    let context = ArithmeticContext::new()?;
    let limits = budget().limits();
    for (policy, refusal) in [
        (PiLimits { work: 0, ..limits }, PiError::WorkLimit),
        (PiLimits { steps: 0, ..limits }, PiError::StepLimit),
        (
            PiLimits {
                staged_bytes: 0,
                ..limits
            },
            PiError::StagedBytesLimit,
        ),
        (
            PiLimits {
                output_bytes: 0,
                ..limits
            },
            PiError::OutputBytesLimit,
        ),
    ] {
        let mut ledger = PiBudget::new(policy);
        let before = ledger.usage();
        assert_eq!(
            PiEnclosure::new(Finite::new(1e-14_f64)?, &context, &mut ledger),
            Err(refusal)
        );
        assert_eq!(ledger.usage(), before);
    }
    Ok(())
}

fn exact_boundary<P: Binary>(tolerance: P, expected: PiUsage) -> TestResult {
    let context = ArithmeticContext::new()?;
    let limits = PiLimits {
        work: expected.work,
        steps: expected.steps,
        staged_bytes: expected.staged_bytes,
        output_bytes: expected.output_bytes,
    };
    let mut ledger = PiBudget::new(limits);
    let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger)?;
    assert_eq!(ledger.usage(), expected);
    assert_eq!(result.work_used(), expected.work);
    assert_eq!(u64::from(result.steps_used()), expected.steps);
    check(result, tolerance);
    for (policy, refusal) in [
        (
            PiLimits {
                work: limits.work - 1,
                ..limits
            },
            PiError::WorkLimit,
        ),
        (
            PiLimits {
                steps: limits.steps.saturating_sub(1),
                ..limits
            },
            PiError::StepLimit,
        ),
        (
            PiLimits {
                staged_bytes: limits.staged_bytes - 1,
                ..limits
            },
            PiError::StagedBytesLimit,
        ),
        (
            PiLimits {
                output_bytes: limits.output_bytes - 1,
                ..limits
            },
            PiError::OutputBytesLimit,
        ),
    ] {
        let seed_step = refusal == PiError::StepLimit && limits.steps == 0;
        if seed_step {
            continue;
        }
        let mut refused = PiBudget::new(policy);
        let before = refused.usage();
        assert_eq!(
            PiEnclosure::new(Finite::new(tolerance)?, &context, &mut refused),
            Err(refusal)
        );
        assert_eq!(refused.usage(), before);
    }
    Ok(())
}

fn cumulative_boundary<P: Binary>(tolerance: P, limits: PiLimits, first: PiUsage) -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = PiBudget::new(limits);
    PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger)?;
    assert_eq!(ledger.usage(), first);
    PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger)?;
    assert_eq!(
        ledger.usage(),
        PiUsage {
            work: limits.work,
            steps: limits.steps,
            staged_bytes: limits.staged_bytes,
            output_bytes: limits.output_bytes
        }
    );
    for (policy, refusal) in [
        (
            PiLimits {
                work: limits.work - 1,
                ..limits
            },
            PiError::WorkLimit,
        ),
        (
            PiLimits {
                steps: limits.steps.saturating_sub(1),
                ..limits
            },
            PiError::StepLimit,
        ),
        (
            PiLimits {
                output_bytes: limits.output_bytes - 1,
                ..limits
            },
            PiError::OutputBytesLimit,
        ),
    ] {
        let seed_step = refusal == PiError::StepLimit && limits.steps == 0;
        if seed_step {
            continue;
        }
        let mut refused = PiBudget::new(policy);
        PiEnclosure::new(Finite::new(tolerance)?, &context, &mut refused)?;
        let before = refused.usage();
        assert_eq!(
            PiEnclosure::new(Finite::new(tolerance)?, &context, &mut refused),
            Err(refusal)
        );
        assert_eq!(refused.usage(), before);
    }
    Ok(())
}

/// Literal Work, steps and byte boundaries cover the seed and both binary formats.
/// ´claim:scalar:pi-resource-boundaries´
/// ´test:integration:pi-resource-boundaries´
#[test]
fn pi_resource_boundaries() -> TestResult {
    exact_boundary(
        2.0_f32,
        PiUsage {
            work: 93,
            steps: 0,
            staged_bytes: 240,
            output_bytes: 64,
        },
    )?;
    exact_boundary(
        2.0_f64,
        PiUsage {
            work: 151,
            steps: 0,
            staged_bytes: 256,
            output_bytes: 80,
        },
    )?;
    for tolerance in [0.000_001_f32, 0.000_000_3] {
        exact_boundary(
            tolerance,
            PiUsage {
                work: 668,
                steps: 5,
                staged_bytes: 240,
                output_bytes: 64,
            },
        )?;
    }
    for (tolerance, work, steps) in [(1e-12_f64, 1708, 9), (1e-14, 1881, 10), (5e-16, 2054, 11)] {
        exact_boundary(
            tolerance,
            PiUsage {
                work,
                steps,
                staged_bytes: 256,
                output_bytes: 80,
            },
        )?;
    }
    Ok(())
}

/// Repeated requests charge full cumulative Work, steps and output with atomic late refusal.
/// ´claim:scalar:pi-cumulative-resource-boundaries´
/// ´test:integration:pi-cumulative-resource-boundaries´
#[test]
fn pi_cumulative_resource_boundaries() -> TestResult {
    cumulative_boundary(
        0.000_000_3_f32,
        PiLimits {
            work: 1336,
            steps: 10,
            staged_bytes: 240,
            output_bytes: 128,
        },
        PiUsage {
            work: 668,
            steps: 5,
            staged_bytes: 240,
            output_bytes: 64,
        },
    )?;
    cumulative_boundary(
        5e-16_f64,
        PiLimits {
            work: 4108,
            steps: 22,
            staged_bytes: 256,
            output_bytes: 160,
        },
        PiUsage {
            work: 2054,
            steps: 11,
            staged_bytes: 256,
            output_bytes: 80,
        },
    )?;
    Ok(())
}

/// A larger-format staging refusal preserves a previously committed smaller-format result.
/// ´claim:scalar:pi-mixed-format-staging-boundary´
/// ´test:integration:pi-mixed-format-staging-boundary´
#[test]
fn pi_mixed_format_staging_boundary() -> TestResult {
    let context = ArithmeticContext::new()?;
    for staged_bytes in [256, 255] {
        let mut ledger = PiBudget::new(PiLimits {
            work: 2722,
            steps: 16,
            staged_bytes,
            output_bytes: 144,
        });
        PiEnclosure::new(Finite::new(0.000_000_3_f32)?, &context, &mut ledger)?;
        let before = ledger.usage();
        let result = PiEnclosure::new(Finite::new(5e-16_f64)?, &context, &mut ledger);
        if staged_bytes == 256 {
            result?;
            assert_eq!(
                ledger.usage(),
                PiUsage {
                    work: 2722,
                    steps: 16,
                    staged_bytes: 256,
                    output_bytes: 144
                }
            );
        } else {
            assert_eq!(result, Err(PiError::StagedBytesLimit));
            assert_eq!(ledger.usage(), before);
        }
    }
    Ok(())
}
