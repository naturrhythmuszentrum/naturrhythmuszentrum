// SPDX-License-Identifier: AGPL-3.0-only
//! Independent cumulative exact and numerical resource ceilings for the closed coefficient recipe.

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::{plan, proved, shapes};
use nrz_bindings::{
    Budget, Limits,
    readings::{PositiveReciprocalMap, algebra::Location},
};
use nrz_matrix::OperationDraft;
use nrz_scalar::LiteralLimits;

/// Both public operations pin cumulative work and independent staged/output byte ceilings.
/// ´test:integration:reciprocal-operator-resource-boundaries´
#[test]
fn reciprocal_operator_resource_boundaries() -> TestResult {
    for (operation, profile) in [
        (0, [810_629, 134_652, 45_762]),
        (1, [107_883, 72_458, 52_211]),
    ] {
        boundaries(operation, profile)?;
    }
    Ok(())
}

/// Every N succeeds, every N−1 refuses, and earlier caller work remains consumed.
fn boundaries(operation: u32, [work, staged, output]: [u64; 3]) -> TestResult {
    for resource in 0..3 {
        let mut limits = Limits::default();
        match resource {
            0 => limits.work = work,
            1 => limits.staged_payload_bytes = staged,
            _ => limits.output_bytes = output,
        }
        assert_eq!(attempt(operation, limits.clone(), 0, 0)?, work);
        match resource {
            0 => limits.work -= 1,
            1 => limits.staged_payload_bytes -= 1,
            _ => limits.output_bytes -= 1,
        }
        assert!(attempt(operation, limits, 0, 0).is_err());
    }
    assert!(
        attempt(
            operation,
            Limits {
                work,
                ..Limits::default()
            },
            1,
            0
        )
        .is_err()
    );
    assert!(
        attempt(
            operation,
            Limits {
                staged_payload_bytes: staged,
                ..Limits::default()
            },
            0,
            1
        )
        .is_err()
    );
    Ok(())
}

/// Prepare inputs independently before constraining the measured public operation.
fn attempt(operation: u32, limits: Limits, precharge: u64, retained: u64) -> TestResult<u64> {
    let mut budget = Budget::new(limits)?;
    budget.work(precharge, Location::new("reciprocal precharge"))?;
    budget.with_retained_payload(
        retained,
        Location::new("reciprocal retained caller"),
        |budget| run(operation, budget),
    )?;
    Ok(budget.usage().work)
}

/// Inputs are prepared with independent ledgers; the measured operation shares its caller's budget.
fn run(operation: u32, budget: &mut Budget) -> TestResult {
    if operation == 0 {
        let mut prepared = plan()?;
        PositiveReciprocalMap::prove(
            &mut prepared.case.cas,
            &prepared.frame,
            prepared.definition,
            budget,
        )?;
    } else {
        let (prepared, recipe) = proved()?;
        let (inputs, output) = shapes(&recipe)?;
        OperationDraft::positive_reciprocal_map(
            &prepared.case.cas,
            recipe,
            inputs,
            output,
            LiteralLimits::default(),
            budget,
        )?;
    }
    Ok(())
}
