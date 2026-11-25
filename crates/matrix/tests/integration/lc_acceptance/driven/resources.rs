// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped continuation work remains cumulative beside independently retained invariant results.

use super::{Status, Stratum, TestResult, execute, ledger, setup};
use nrz_bindings::{Budget, readings::algebra::Location};
use nrz_scalar::Scalar;

/// Exhaustion before a later decision leaves the root and consumed prefix intact, without retry.
/// ´test:integration:lc-driven-continuation-cumulative-resource-stop´
#[test]
fn lc_driven_continuation_cumulative_resource_stop() -> TestResult {
    refusal::<f32>()?;
    refusal::<f64>()
}

/// An already consumed caller prefix leaves no Work for the first decision after the root.
fn refusal<P: Scalar + 'static>() -> TestResult {
    let mut original = setup::<P>(false)?;
    execute(
        &mut original,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    let invariant_work = original
        .batch
        .usage(Stratum::Invariant)
        .ok_or("invariant receipt")?[1]
        .work;
    let mut case = setup::<P>(false)?;
    let mut policy = crate::phasor_solve_testing::policy();
    policy.work = invariant_work + 1;
    let mut budget = Budget::new(policy)?;
    budget.work(1, Location::new("driven consumed prefix"))?;
    execute(&mut case, &mut budget)?;
    assert_eq!(
        case.batch.statuses(0),
        Some([Status::ResourceRefused, Status::Blocked, Status::Blocked])
    );
    for row in [1, 2] {
        assert_eq!(case.batch.statuses(row), Some([Status::Blocked; 3]));
    }
    assert!(budget.usage().work >= invariant_work);
    assert!(case.batch.failure(0, Stratum::Decision).is_some());
    case.batch
        .invariants()
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    assert!(
        execute(
            &mut case,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?
        )
        .is_err()
    );
    let inventory = case.batch.inventory();
    assert_eq!(inventory.points, 3);
    assert!(inventory.observed_capacity_bytes >= inventory.logical_payload_bytes);
    assert_eq!(inventory.logical_payload_bytes, 7216);
    assert_eq!(inventory.observed_capacity_bytes, 7216);

    Ok(())
}
