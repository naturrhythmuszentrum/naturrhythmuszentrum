// SPDX-License-Identifier: AGPL-3.0-only
//! Literal caller and retained-byte boundaries for each exercised signed exponent.

use super::{Pair, TestResult, pair};
use nrz_bindings::{Budget, Descriptor, Limits};
use nrz_matrix::{Quantity, QuantityError};

/// Minimum cold allowance, actual cold charge and warm caller charge by exponent.
const COSTS: [(i64, u64, u64, u64); 6] = [
    (-2, 1_140_885, 936_741, 11_185),
    (-1, 1_101_535, 899_752, 10_140),
    (0, 350_585, 326_066, 9_657),
    (1, 282_273, 270_927, 9_621),
    (2, 764_159, 657_793, 10_666),
    (3, 501_507, 476_596, 11_679),
];

/// Literal cold and warm retained provenance byte totals by exponent.
const BYTES: [(i64, usize, usize); 6] = [
    (-2, 2_441, 4_882),
    (-1, 2_129, 4_258),
    (0, 2_129, 4_258),
    (1, 1_817, 3_634),
    (2, 2_129, 4_258),
    (3, 2_441, 4_882),
];

/// Every boundary starts in a fresh store with only its stated warm-up.
fn fixture(exponent: i64, warm: bool) -> TestResult<Pair<f64>> {
    let ratio = Descriptor::ratio();
    let mut case = pair::<f64>([&ratio, &ratio])?;
    if warm {
        execute(&mut case, exponent, &mut Budget::new(Limits::default())?)?;
    }
    Ok(case)
}

/// Exercise the public facade without changing the represented operating point.
fn execute(
    case: &mut Pair<f64>,
    exponent: i64,
    budget: &mut Budget,
) -> Result<Quantity<f64>, QuantityError<f64>> {
    case.context
        .integer_power(&mut case.heap, &case.values[0], exponent, budget)
}

/// Cold construction and warm numerical execution have distinct literal Work boundaries.
/// ´test:integration:quantity-power-covariance-work-boundaries´
/// ´claim:matrix:quantity-power-covariance-work-boundaries´ Signed, zero and unit power covariance accepts literal cold and warm Work allowances, preserves the measured charges and refuses one below with named exhaustion, unchanged input versions and no publication.
#[test]
fn quantity_power_covariance_work_boundaries() -> TestResult {
    for (exponent, cold, charge, hot) in COSTS {
        for warm in [false, true] {
            let (required, charged) = if warm { (hot, hot) } else { (cold, charge) };
            for accepted in [false, true] {
                let mut case = fixture(exponent, warm)?;
                let before = case.heap.provenance_usage();
                let stamp = case.values[0].view(&case.heap)?.stamp();
                let mut budget = Budget::new(Limits {
                    work: if accepted { required } else { required - 1 },
                    ..Limits::default()
                })?;
                let result = execute(&mut case, exponent, &mut budget);
                if accepted {
                    result?;
                    assert_eq!(budget.usage().work, charged);
                } else {
                    let Err(fault) = result else {
                        panic!("one-below quantity power Work must refuse");
                    };
                    exhaustion(fault, warm, required);
                    assert!(budget.usage().work > 0);
                    assert!(budget.usage().work < required);
                    assert_eq!(case.heap.provenance_usage(), before);
                }
                assert_eq!(case.values[0].view(&case.heap)?.stamp(), stamp);
            }
        }
    }
    Ok(())
}

/// Preserve the named owner of a disjoint symbolic reservation or final caller charge.
fn exhaustion(fault: QuantityError<f64>, warm: bool, required: u64) {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    if warm {
        let QuantityError::Resources(fault) = fault else {
            panic!("warm power must retain caller exhaustion");
        };
        let FaultKind::Exhausted {
            budget: BudgetKind::Work,
            limit,
            needed,
            ..
        } = fault.kind
        else {
            panic!("warm power must exhaust Work");
        };
        assert_eq!(limit, required - 1);
        assert_eq!(needed, required);
    } else {
        let QuantityError::Operation(fault) = fault else {
            panic!("cold power must retain its operation refusal");
        };
        let nrz_matrix::OperationError::Statistics(nrz_matrix::StatisticsError::Sensitivity(cause)) =
            *fault
        else {
            panic!("cold power must retain its symbolic cause");
        };
        let nrz_bindings::SensitivityError::Exhausted { fault, outcome } = cause.as_ref() else {
            panic!("cold symbolic allowance must expose exhaustion");
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work | BudgetKind::ReplayWork,
                ..
            }
        ));
        assert!(matches!(
            outcome.as_ref(),
            nrz_bindings::readings::algebra::request::Outcome::Exhausted { .. }
        ));
    }
}

/// Retained output bytes have fresh cold and explicitly warmed one-byte refusal twins.
/// ´test:integration:quantity-power-covariance-provenance-boundaries´
/// ´claim:matrix:quantity-power-covariance-provenance-boundaries´ Signed, zero and unit power covariance accepts its literal cold and warm provenance byte limits and refuses one byte below atomically while preserving spent Work and original values.
#[test]
fn quantity_power_covariance_provenance_boundaries() -> TestResult {
    for (exponent, cold, hot) in BYTES {
        for warm in [false, true] {
            let required = if warm { hot } else { cold };
            for accepted in [false, true] {
                let mut case = fixture(exponent, warm)?;
                case.heap
                    .set_provenance_limits(nrz_matrix::ProvenanceLimits::new(
                        1024,
                        16_384,
                        65_536,
                        if accepted { required } else { required - 1 },
                        1_048_576,
                    ))?;
                let before = case.heap.provenance_usage();
                let mut budget = Budget::new(Limits::default())?;
                let result = execute(&mut case, exponent, &mut budget);
                if accepted {
                    result?;
                    assert_eq!(case.heap.provenance_usage().bytes(), required);
                } else {
                    let Err(QuantityError::Execution(failure)) = result else {
                        panic!("one-below quantity power bytes must refuse");
                    };
                    assert!(matches!(
                        failure.cause(),
                        nrz_matrix::OperationError::ProvenanceLimit
                    ));
                    assert_eq!(case.heap.provenance_usage(), before);
                    assert!(budget.usage().work > 0);
                }
                assert_eq!(case.values[0].view(&case.heap)?.region().get(0, 0)?, 3.0);
            }
        }
    }
    Ok(())
}
