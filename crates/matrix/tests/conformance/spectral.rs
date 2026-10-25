// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical spectral admission and execution retain explicit cumulative Work and byte limits.

use crate::spectral_testing::{Fixture, TestResult, ledger};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_matrix::{Heap, LcSpectrum};
use nrz_scalar::ArithmeticContext;

/// Planning and execution share prior Work; an exhausted retry cannot obtain a new default ledger.
/// ´test:integration:lc-spectral-numerical-work-boundaries´
#[test]
fn lc_spectral_numerical_work_boundaries() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let (heap, inputs) = fixture.import([[1.0_f64, 0.125, 0.0]; 2])?;
    let context = ArithmeticContext::new()?;
    let mut measured = ledger()?;
    measured.work(17, Location::new("prior spectral work"))?;
    let plan = fixture.plan(inputs.clone(), &mut measured)?;
    heap.evaluate_lc_spectrum(&plan, &context, &mut measured)?;
    let required = measured.usage().work;
    for work in [required, required - 1] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, Location::new("prior spectral work"))?;
        let plan = fixture.plan(inputs.clone(), &mut budget)?;
        let result = heap.evaluate_lc_spectrum(&plan, &context, &mut budget);
        assert_eq!(result.is_ok(), work == required, "{result:?}");
        if result.is_err() {
            let spent = budget.usage();
            assert!(
                heap.evaluate_lc_spectrum(&plan, &context, &mut budget)
                    .is_err()
            );
            assert!(budget.usage().work >= spent.work);
        }
    }
    Ok(())
}

/// Owned output and scratch allocations have independent exact limits with caller retention.
/// ´test:integration:lc-spectral-numerical-payload-boundaries´
#[test]
fn lc_spectral_numerical_payload_boundaries() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let (heap, inputs) = fixture.import([[1.0_f64, 0.125, 0.0]; 2])?;
    let plan = fixture.plan(inputs.clone(), &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    for (kind, requirements) in [
        (BudgetKind::StagedPayloadBytes, [87_803, 1064]),
        (BudgetKind::OutputBytes, [14_793, 680]),
    ] {
        for (planning, required) in [true, false].into_iter().zip(requirements) {
            let attempt = |limit, prior| -> TestResult<bool> {
                let mut limits = Limits::default();
                match kind {
                    BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
                    _ => limits.output_bytes = limit,
                }
                let mut budget = Budget::new(limits)?;
                Ok(budget
                    .with_retained_payload(
                        prior,
                        Location::new("retained caller payload"),
                        |budget| {
                            if planning {
                                fixture.plan(inputs.clone(), budget).map(|_| ())
                            } else {
                                execute(&heap, &plan, &context, budget)
                            }
                        },
                    )
                    .is_ok())
            };
            assert!(attempt(required, 0)?);
            assert!(!attempt(required - 1, 0)?);
            if kind == BudgetKind::StagedPayloadBytes {
                assert!(attempt(required + 23, 23)?);
                assert!(!attempt(required + 22, 23)?);
            }
        }
    }
    Ok(())
}

fn execute(
    heap: &Heap,
    plan: &LcSpectrum<f64>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> TestResult {
    heap.evaluate_lc_spectrum(plan, context, budget)?;
    Ok(())
}
