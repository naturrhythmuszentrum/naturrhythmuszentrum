// SPDX-License-Identifier: AGPL-3.0-only
//! Literal cumulative attempt and retained-payload boundaries measured on the seed.

use super::fixture::{TestResult, enclosure, fixture, fixture_with_budget};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};

/// Independent original cases and measured declaration/attempt/staging/output costs.
const CASES: [(i64, &str, &str, [u64; 4]); 4] = [
    (1000, "-301/4", "-299/4", [131_766, 306_499, 56_880, 31_104]),
    (2000, "0", "0", [128_791, 196_665, 54_900, 14_372]),
    (2000, "-1/4", "1/4", [128_791, 50_634, 29_175, 1300]),
    (4000, "299/4", "301/4", [131_286, 304_489, 56_880, 31_104]),
];
/// Caller work and live payload exist before this consumer begins.
const PRIOR: u64 = 17;
/// Prior caller accounting has an independent location.
const AT: Location = Location::new("axis-phase prior caller prefix");

/// Every original declaration and outcome has literal N/N-1 cumulative Work boundaries.
/// ´test:integration:bindings-lc-axis-phase-cumulative-work´
#[test]
fn bindings_lc_axis_phase_cumulative_work() -> TestResult {
    for (frequency, lower, upper, [declaration, attempt, _, bytes]) in CASES {
        for short in [0, 1] {
            let mut budget = Budget::new(Limits {
                work: PRIOR + declaration - short,
                ..Limits::default()
            })?;
            budget.work(PRIOR, AT)?;
            let result = fixture_with_budget(frequency, &mut budget);
            assert_eq!(result.is_ok(), short == 0);
            assert!(budget.usage().work > PRIOR);
            if let Ok(source) = result {
                assert_eq!(source.case.retained_payload_bytes(), 16_051);
                assert_eq!(budget.usage().work, PRIOR + declaration);
            }
            let source = fixture(frequency)?;
            let mut attempts = source.case.attempts(1);
            let mut budget = Budget::new(Limits {
                work: PRIOR + attempt - short,
                ..Limits::default()
            })?;
            budget.work(PRIOR, AT)?;
            let result = attempts.attempt(&enclosure(&source, lower, upper)?, &mut budget);
            assert_eq!(result.is_ok(), short == 0, "{result:?}");
            assert_eq!(attempts.used(), 1);
            assert!(budget.usage().work > PRIOR);
            if let Ok(result) = result {
                assert_eq!(result.usage()[0].work, PRIOR);
                assert_eq!(result.usage()[1].work, PRIOR + attempt);
                assert_eq!(budget.usage().work, PRIOR + attempt);
                assert_eq!(result.retained_payload_bytes(), bytes);
            }
        }
    }
    Ok(())
}

/// Live payload reduces staged capacity; output has its own unchanged literal boundary.
/// ´test:integration:bindings-lc-axis-phase-payload-boundaries´
#[test]
fn bindings_lc_axis_phase_payload_boundaries() -> TestResult {
    for (frequency, lower, upper, [_, _, staged, output]) in CASES {
        for staging in [true, false] {
            for prior in [0, PRIOR] {
                for short in [0, 1] {
                    let source = fixture(frequency)?;
                    let mut attempts = source.case.attempts(1);
                    let mut limits = Limits::default();
                    if staging {
                        limits.staged_payload_bytes = staged + prior - short;
                    } else {
                        limits.output_bytes = output - short;
                    }
                    let mut budget = Budget::new(limits)?;
                    budget.work(PRIOR, AT)?;
                    let result = budget.with_retained_payload(prior, AT, |budget| {
                        attempts
                            .attempt(&enclosure(&source, lower, upper)?, budget)
                            .map_err(Into::<Box<dyn std::error::Error>>::into)
                    });
                    assert_eq!(result.is_ok(), short == 0, "{result:?}");
                    assert_eq!(attempts.used(), 1);
                    assert!(budget.usage().work > PRIOR);
                }
            }
        }
    }
    Ok(())
}
