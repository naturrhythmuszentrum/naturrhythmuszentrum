// SPDX-License-Identifier: AGPL-3.0-only
//! Measured caller and ordinary provenance boundaries for statistical quantity products.

use super::{TestResult, pair};
use nrz_bindings::{Budget, Descriptor, Limits};

/// Each complete boundary has a fresh model and only the stated compatible warm-up.
/// ´test:integration:quantity-product-covariance-work-boundaries´
/// ´claim:matrix:quantity-product-covariance-work-boundaries´ Cold and warm quantity products accept their literal Work allowances, retain exact consumed charges and refuse one unit below with named exhaustion and no publication prefix.
#[test]
fn quantity_product_covariance_work_boundaries() -> TestResult {
    for (warm, required, charged, refused) in [
        (false, 878_476, 831_948, 722_279),
        (true, 10_443, 10_443, 8_762),
    ] {
        for accepted in [false, true] {
            let ratio = Descriptor::ratio();
            let mut case = pair::<f64>([&ratio, &ratio])?;
            if warm {
                case.context.multiply(
                    &mut case.heap,
                    &case.values[0],
                    &case.values[1],
                    &mut Budget::new(Limits::default())?,
                )?;
            }
            let before = case.heap.provenance_usage();
            let versions = [
                case.values[0].view(&case.heap)?.stamp(),
                case.values[1].view(&case.heap)?.stamp(),
            ];
            let mut budget = Budget::new(Limits {
                work: if accepted { required } else { required - 1 },
                ..Limits::default()
            })?;
            let result = case.context.multiply(
                &mut case.heap,
                &case.values[0],
                &case.values[1],
                &mut budget,
            );
            if accepted {
                result?;
                assert_eq!(budget.usage().work, charged);
            } else {
                let Err(fault) = result else {
                    panic!("one-below product covariance Work must refuse");
                };
                exhaustion(fault, warm);
                assert_eq!(budget.usage().work, refused);
                assert_eq!(case.heap.provenance_usage(), before);
            }
            assert_eq!(
                [
                    case.values[0].view(&case.heap)?.stamp(),
                    case.values[1].view(&case.heap)?.stamp()
                ],
                versions
            );
        }
    }
    Ok(())
}

/// Cold symbolic and warm numerical admission retain different owners of the same named resource.
fn exhaustion(fault: nrz_matrix::QuantityError<f64>, warm: bool) {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    if warm {
        let nrz_matrix::QuantityError::Resources(fault) = fault else {
            panic!("warm arithmetic must expose its caller exhaustion");
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                limit: 10_442,
                needed: 10_443,
                ..
            }
        ));
    } else {
        let nrz_matrix::QuantityError::Operation(fault) = fault else {
            panic!("cold derivative preparation must retain its operation cause");
        };
        let nrz_matrix::OperationError::Statistics(nrz_matrix::StatisticsError::Sensitivity(cause)) =
            *fault
        else {
            panic!("cold statistical sensitivity cause required");
        };
        let nrz_bindings::SensitivityError::Exhausted { fault, outcome } = cause.as_ref() else {
            panic!("cold allowance shortfall must be named exhaustion");
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

/// Retained operation bytes have independent literal cold and warm fresh-store twins.
/// ´test:integration:quantity-product-covariance-provenance-boundaries´
/// ´claim:matrix:quantity-product-covariance-provenance-boundaries´ Cold and warm product covariance publication succeeds at its literal retained provenance byte limit and refuses one byte below without publishing a value or statistical prefix.
#[test]
fn quantity_product_covariance_provenance_boundaries() -> TestResult {
    for (warm, required) in [(false, 2_873), (true, 5_746)] {
        for accepted in [false, true] {
            let ratio = Descriptor::ratio();
            let mut case = pair::<f64>([&ratio, &ratio])?;
            if warm {
                case.context.multiply(
                    &mut case.heap,
                    &case.values[0],
                    &case.values[1],
                    &mut Budget::new(Limits::default())?,
                )?;
            }
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
            let result = case.context.multiply(
                &mut case.heap,
                &case.values[0],
                &case.values[1],
                &mut budget,
            );
            if accepted {
                let value = result?;
                assert_eq!(case.heap.provenance_usage().bytes(), required);
                let target = case
                    .heap
                    .companion(value.view(&case.heap)?.bound().ok_or("bound")?)?
                    .target();
                let moments = case.heap.moments(case.heap.moment_companion(target)?)?;
                assert_eq!(moments.allocation_inventory().metadata_bytes(), 5_240);
                assert_eq!(moments.allocation_inventory().peak_payload_bytes(), 432);
            } else {
                let Err(nrz_matrix::QuantityError::Execution(failure)) = result else {
                    panic!("one-below product covariance provenance bytes must refuse");
                };
                assert!(matches!(
                    failure.cause(),
                    nrz_matrix::OperationError::ProvenanceLimit
                ));
                assert_eq!(case.heap.provenance_usage(), before);
                assert!(budget.usage().work > 0);
            }
            assert_eq!(case.values[0].view(&case.heap)?.region().get(0, 0)?, 3.0);
            assert_eq!(case.values[1].view(&case.heap)?.region().get(0, 0)?, 2.0);
        }
    }
    Ok(())
}
