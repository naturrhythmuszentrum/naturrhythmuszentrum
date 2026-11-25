// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational pi witnesses, monotone attempt spend and atomic output.

use crate::testing::{Binary, Rational};
use nrz_scalar::{
    ArithmeticContext, Finite, PiBounds, PiBudget, PiEnclosure, PiError, PiLimits, PiUsage,
};
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const fn fresh_budget() -> PiBudget {
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
        let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut fresh_budget())?;
        check(result, tolerance);
    }
    for tolerance in [2.0_f64, 0.01, 1e-12, 1e-14, 5e-16] {
        let result = PiEnclosure::new(Finite::new(tolerance)?, &context, &mut fresh_budget())?;
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
    let mut ledger = fresh_budget();
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

/// Invalid tolerances cost nothing; exhausted resolution retains Work and steps, not output.
/// ´claim:scalar:pi-tolerance-refusals-are-atomic´
/// ´test:integration:pi-tolerance-refusals-are-atomic´
#[test]
fn pi_tolerance_refusals_are_atomic() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = fresh_budget();
    PiEnclosure::new(Finite::new(2.0_f64)?, &context, &mut ledger)?;
    assert_eq!(ledger.usage().work, 151);
    for (tolerance, refusal, work, steps) in [
        (0.0_f64, PiError::InvalidTolerance, 151, 0),
        (-1.0, PiError::InvalidTolerance, 151, 0),
        (f64::MIN_POSITIVE, PiError::ToleranceUnreachable, 3935, 21),
    ] {
        let before = ledger.usage();
        assert_eq!(
            PiEnclosure::new(Finite::new(tolerance)?, &context, &mut ledger),
            Err(refusal)
        );
        assert_eq!(
            ledger.usage(),
            PiUsage {
                work,
                steps,
                ..before
            }
        );
    }
    let before = ledger.usage();
    assert_eq!(
        PiEnclosure::new(Finite::new(f32::MIN_POSITIVE)?, &context, &mut ledger),
        Err(PiError::ToleranceUnreachable)
    );
    assert_eq!(
        ledger.usage(),
        PiUsage {
            work: 6443,
            steps: 42,
            ..before
        }
    );
    Ok(())
}

/// Independent caps refuse before publication even with a zero allowance.
/// ´claim:scalar:pi-zero-resource-refusals´
/// ´test:integration:pi-zero-resource-refusals´
#[test]
fn pi_zero_resource_refusals() -> TestResult {
    let context = ArithmeticContext::new()?;
    let limits = fresh_budget().limits();
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
        let work = if refusal == PiError::StepLimit {
            151
        } else {
            0
        };
        assert_eq!(ledger.usage(), PiUsage { work, ..before });
    }
    Ok(())
}

fn exact_boundary<P: Binary>(tolerance: P, expected: PiUsage, failed_work: [u64; 2]) -> TestResult {
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
        let (work, steps) = match refusal {
            PiError::WorkLimit => (failed_work[0], limits.steps),
            PiError::StepLimit => (failed_work[1], limits.steps.saturating_sub(1)),
            _ => (before.work, before.steps),
        };
        assert_eq!(
            refused.usage(),
            PiUsage {
                work,
                steps,
                ..before
            }
        );
    }
    Ok(())
}

fn cumulative_boundary<P: Binary>(
    tolerance: P,
    limits: PiLimits,
    first: PiUsage,
    failed_work: [u64; 2],
) -> TestResult {
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
        let (work, steps) = match refusal {
            PiError::WorkLimit => (failed_work[0], limits.steps),
            PiError::StepLimit => (failed_work[1], limits.steps.saturating_sub(1)),
            _ => (before.work, before.steps),
        };
        assert_eq!(
            refused.usage(),
            PiUsage {
                work,
                steps,
                ..before
            }
        );
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
        [92, 0],
    )?;
    exact_boundary(
        2.0_f64,
        PiUsage {
            work: 151,
            steps: 0,
            staged_bytes: 256,
            output_bytes: 80,
        },
        [150, 0],
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
            [667, 553],
        )?;
    }
    for (tolerance, work, steps, failed_work) in [
        (1e-12_f64, 1708, 9, [1707, 1535]),
        (1e-14, 1881, 10, [1880, 1708]),
        (5e-16, 2054, 11, [2053, 1881]),
    ] {
        exact_boundary(
            tolerance,
            PiUsage {
                work,
                steps,
                staged_bytes: 256,
                output_bytes: 80,
            },
            failed_work,
        )?;
    }
    Ok(())
}

/// Repeated requests retain attempted Work and steps while late refusal preserves prior output.
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
        [1335, 1221],
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
        [4107, 3935],
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

/// Finite failed attempts exhaust one original ledger without publishing any additional output.
/// ´claim:scalar:pi-repeated-resolution-failures-exhaust-original-ledger´
/// ´test:integration:pi-repeated-resolution-failures-exhaust-original-ledger´
#[test]
fn pi_repeated_resolution_failures_exhaust_original_ledger() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = PiBudget::new(PiLimits {
        work: 7724,
        steps: 42,
        staged_bytes: 256,
        output_bytes: 160,
    });
    let retained = PiEnclosure::new(Finite::new(2.0_f64)?, &context, &mut ledger)?;
    assert_eq!(ledger.usage().work, 151);
    for (work, steps, refusal) in [
        (3935, 21, PiError::ToleranceUnreachable),
        (7719, 42, PiError::ToleranceUnreachable),
        (7724, 42, PiError::WorkLimit),
        (7724, 42, PiError::WorkLimit),
    ] {
        assert_eq!(
            PiEnclosure::new(Finite::new(f64::MIN_POSITIVE)?, &context, &mut ledger),
            Err(refusal)
        );
        assert_eq!(
            ledger.usage(),
            PiUsage {
                work,
                steps,
                staged_bytes: 256,
                output_bytes: 80,
            }
        );
    }
    check(retained, 2.0);
    Ok(())
}

/// A failed step reservation does not consume a step; temporary storage remains reusable.
/// ´claim:scalar:pi-step-work-is-admitted-together-and-storage-is-released´
/// ´test:integration:pi-step-work-is-admitted-together-and-storage-is-released´
#[test]
fn pi_step_work_is_admitted_together_and_storage_is_released() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut ledger = PiBudget::new(PiLimits {
        work: 176,
        steps: 1,
        staged_bytes: 256,
        output_bytes: 80,
    });
    assert_eq!(
        PiEnclosure::new(Finite::new(1e-14_f64)?, &context, &mut ledger),
        Err(PiError::WorkLimit)
    );
    assert_eq!(
        ledger.usage(),
        PiUsage {
            work: 151,
            steps: 0,
            ..PiUsage::default()
        }
    );
    let mut ledger = PiBudget::new(PiLimits {
        work: 302,
        steps: 0,
        staged_bytes: 256,
        output_bytes: 80,
    });
    assert_eq!(
        PiEnclosure::new(Finite::new(1e-14_f64)?, &context, &mut ledger),
        Err(PiError::StepLimit)
    );
    assert_eq!(
        ledger.usage(),
        PiUsage {
            work: 151,
            ..PiUsage::default()
        }
    );
    let result = PiEnclosure::new(Finite::new(2.0_f64)?, &context, &mut ledger)?;
    assert_eq!(result.work_used(), 151);
    assert_eq!(
        ledger.usage(),
        PiUsage {
            work: 302,
            steps: 0,
            staged_bytes: 256,
            output_bytes: 80
        }
    );
    Ok(())
}
