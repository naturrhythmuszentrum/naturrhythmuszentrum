// SPDX-License-Identifier: AGPL-3.0-only
//! Quotient and positive-root proof allowances remain distinct from consumed charges.

use super::{Pair, TestResult, pair};
use nrz_bindings::{Budget, Descriptor, Limits};
use nrz_matrix::{Quantity, QuantityError};

/// Select the independently checked quotient or positive-root recipe.
#[derive(Clone, Copy)]
enum Recipe {
    /// Ordered quotient of the original represented pair.
    Quotient,
    /// Positive root of the first original represented coordinate.
    Root,
}

/// Minimum cold allowance, actual charge and warm charge for each recipe.
const COSTS: [(Recipe, u64, u64, u64); 2] = [
    (Recipe::Quotient, 1_931_495, 1_694_935, 10_991),
    (Recipe::Root, 1_377_419, 1_277_836, 9_632),
];

/// Literal cold and warm retained provenance totals.
const BYTES: [(Recipe, usize, usize); 2] = [
    (Recipe::Quotient, 3_185, 6_370),
    (Recipe::Root, 2_129, 4_258),
];

/// Every boundary starts in a fresh store with only its stated warm-up.
fn fixture(recipe: Recipe, warm: bool) -> TestResult<Pair<f64>> {
    let ratio = Descriptor::ratio();
    let mut case = pair::<f64>([&ratio, &ratio])?;
    if warm {
        execute(&mut case, recipe, &mut Budget::new(Limits::default())?)?;
    }
    Ok(case)
}

/// Exercise the public facade without changing the represented operating point.
fn execute(
    case: &mut Pair<f64>,
    recipe: Recipe,
    budget: &mut Budget,
) -> Result<Quantity<f64>, QuantityError<f64>> {
    match recipe {
        Recipe::Quotient => {
            case.context
                .divide(&mut case.heap, &case.values[0], &case.values[1], budget)
        }
        Recipe::Root => case
            .context
            .square_root(&mut case.heap, &case.values[0], budget),
    }
}

/// Cold construction and warm numerical execution have distinct literal Work boundaries.
/// ´test:integration:quantity-nonlinear-covariance-work-boundaries´
/// ´claim:matrix:quantity-nonlinear-covariance-work-boundaries´ Quotient and positive-root covariance accepts literal cold and warm Work allowances, preserves the measured charges and refuses one below with named exhaustion, unchanged input versions and no publication.
#[test]
fn quantity_nonlinear_covariance_work_boundaries() -> TestResult {
    for (recipe, cold, charge, hot) in COSTS {
        for warm in [false, true] {
            let (required, charged) = if warm { (hot, hot) } else { (cold, charge) };
            for accepted in [false, true] {
                let mut case = fixture(recipe, warm)?;
                let before = case.heap.provenance_usage();
                let stamp = case.values[0].view(&case.heap)?.stamp();
                let mut budget = Budget::new(Limits {
                    work: if accepted { required } else { required - 1 },
                    ..Limits::default()
                })?;
                let result = execute(&mut case, recipe, &mut budget);
                if accepted {
                    result?;
                    assert_eq!(budget.usage().work, charged);
                } else {
                    let Err(fault) = result else {
                        panic!("one-below quantity nonlinear Work must refuse");
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
            panic!("warm nonlinear must retain caller exhaustion");
        };
        let FaultKind::Exhausted {
            budget: BudgetKind::Work,
            limit,
            needed,
            ..
        } = fault.kind
        else {
            panic!("warm nonlinear must exhaust Work");
        };
        assert_eq!(limit, required - 1);
        assert_eq!(needed, required);
    } else {
        let QuantityError::Operation(fault) = fault else {
            panic!("cold nonlinear must retain its operation refusal");
        };
        let nrz_matrix::OperationError::Statistics(nrz_matrix::StatisticsError::Sensitivity(cause)) =
            *fault
        else {
            panic!("cold nonlinear must retain its symbolic cause");
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
/// ´test:integration:quantity-nonlinear-covariance-provenance-boundaries´
/// ´claim:matrix:quantity-nonlinear-covariance-provenance-boundaries´ Quotient and positive-root covariance accepts its literal cold and warm provenance byte limits and refuses one byte below atomically while preserving spent Work and original values.
#[test]
fn quantity_nonlinear_covariance_provenance_boundaries() -> TestResult {
    for (recipe, cold, hot) in BYTES {
        for warm in [false, true] {
            let required = if warm { hot } else { cold };
            for accepted in [false, true] {
                let mut case = fixture(recipe, warm)?;
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
                let result = execute(&mut case, recipe, &mut budget);
                if accepted {
                    result?;
                    assert_eq!(case.heap.provenance_usage().bytes(), required);
                } else {
                    let Err(QuantityError::Execution(failure)) = result else {
                        panic!("one-below quantity nonlinear bytes must refuse");
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
