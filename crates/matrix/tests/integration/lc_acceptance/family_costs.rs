// SPDX-License-Identifier: AGPL-3.0-only
//! Measured consumer ceilings and the complete shared caller ledger have literal boundaries.

use super::{
    family_models as models, family_support as family, reciprocal_statistics as shared,
    root_support as inputs, root_work::policy,
};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::{Budget, readings::algebra::Location};
use nrz_matrix::ReciprocalDerivativeContract;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Every consumer owner admits at N, refuses at N minus one, and preserves earlier caller usage.
/// ´test:integration:lc-operator-root-family-resource-boundaries´
#[test]
fn lc_operator_root_family_resource_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    boundaries::<f32>(&fixture, &contract, false, [216, 2224, 512], &mut budget)?;
    boundaries::<f64>(&fixture, &contract, false, [216, 3008, 1024], &mut budget)?;
    boundaries::<f32>(&fixture, &contract, true, [228, 2224, 512], &mut budget)?;
    boundaries::<f64>(&fixture, &contract, true, [228, 3008, 1024], &mut budget)
}

fn boundaries<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    full_rank: bool,
    required: [u64; 3],
    budget: &mut Budget,
) -> TestResult {
    let mut case = prepare::<P>(fixture, contract, full_rank, budget)?;
    let outputs = family::publish(fixture, &mut case, budget)?;
    let context = ArithmeticContext::new()?;
    for (kind, need) in required.into_iter().enumerate() {
        for (available, success) in [(need, true), (need - 1, false)] {
            let before = case.heap.provenance_usage();
            let mut limit = policy(kind, available, 0)?;
            let result = case.heap.lc_operator_root_covariance(
                outputs.moments,
                &outputs.roots,
                &context,
                &mut limit,
            );
            assert_eq!(result.is_ok(), success, "owner {kind}: {result:?}");
            assert_eq!(case.heap.provenance_usage(), before);
            if success {
                assert_eq!(limit.usage().work, required[0]);
            }
        }
    }
    let before = case.heap.provenance_usage();
    let result = case.heap.lc_operator_root_covariance(
        outputs.moments,
        &outputs.roots,
        &context,
        &mut policy(0, required[0], 1)?,
    );
    assert!(result.is_err());
    assert_eq!(case.heap.provenance_usage(), before);
    let result = policy(1, required[1], 0)?.with_retained_payload(
        1,
        Location::new("earlier retained family owner"),
        |limit| {
            case.heap
                .lc_operator_root_covariance(outputs.moments, &outputs.roots, &context, limit)
        },
    );
    assert!(result.is_err());
    assert_eq!(case.heap.provenance_usage(), before);
    Ok(())
}

/// One cold derivative preparation serves both publications and the final complete family read.
/// ´test:integration:lc-operator-root-family-cumulative-work-boundary´
#[test]
fn lc_operator_root_family_cumulative_work_boundary() -> TestResult {
    cumulative::<f32>(false, 5_694_101)?;
    cumulative::<f64>(false, 5_694_101)?;
    cumulative::<f32>(true, 5_694_145)?;
    cumulative::<f64>(true, 5_694_145)
}

fn cumulative<P: Scalar + 'static>(full_rank: bool, required: u64) -> TestResult {
    for (available, prior, success) in [
        (required, 0, true),
        (required - 1, 0, false),
        (required, 1, false),
    ] {
        let mut limits = shared::policy()?.limits().clone();
        limits.work = available;
        let mut budget = Budget::new(limits)?;
        budget.work(prior, Location::new("earlier integrated family owner"))?;
        let result = lifecycle::<P>(full_rank, &mut budget);
        assert_eq!(result.is_ok(), success, "{result:?}");
        if success {
            assert_eq!(budget.usage().work, required);
        }
    }
    Ok(())
}

/// Existing fixture construction and native execution remain separate from the bindings ledger.
fn lifecycle<P: Scalar + 'static>(full_rank: bool, budget: &mut Budget) -> TestResult {
    let start = budget.usage().work;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, budget)?;
    let mut case = prepare::<P>(&fixture, &contract, full_rank, budget)?;
    let outputs = family::publish(&fixture, &mut case, budget)?;
    let published = if full_rank { 5_693_917 } else { 5_693_885 };
    assert_eq!(budget.usage().work - start, published);
    shared_identity(&case, &outputs, &contract, full_rank)?;
    let before = case.heap.provenance_usage();
    let result = case.heap.lc_operator_root_covariance(
        outputs.moments,
        &outputs.roots,
        &ArithmeticContext::new()?,
        budget,
    );
    assert_eq!(case.heap.provenance_usage(), before);
    result?;
    Ok(())
}

/// Each publication exposes the same exact proof owner and original joint source identities.
fn shared_identity<P: Scalar + 'static>(
    case: &inputs::Case<P>,
    outputs: &family::Outputs<P>,
    contract: &ReciprocalDerivativeContract,
    full_rank: bool,
) -> TestResult {
    let moments = case.heap.moments(outputs.moments)?;
    let inventory = moments.allocation_inventory();
    let operator = inventory
        .operation()
        .and_then(nrz_matrix::CheckedOperation::reciprocal_jacobian)
        .ok_or("native operator derivative")?;
    let plan = outputs.roots.evaluation().plan();
    let root = plan.root_jacobian().ok_or("root derivative")?;
    assert!(std::ptr::eq(operator.native(), contract.exact().native()));
    assert!(std::ptr::eq(
        root.operator().native(),
        contract.exact().native()
    ));
    let (model, sources) = plan.statistical_identity().ok_or("root identity")?;
    assert_eq!(model, moments.model());
    let ordered = if full_rank {
        [sources[1], sources[0]]
    } else {
        sources
    };
    assert_eq!(ordered.as_slice(), moments.source_ids());
    assert_eq!(inventory.metadata_bytes(), 5256);
    assert_eq!(inventory.peak_payload_bytes(), 172 * size_of::<P>());
    let phase = if full_rank { 960 } else { 928 };
    assert_eq!(inventory.backend_allowance_bytes(), phase * size_of::<P>());
    Ok(())
}

/// Each original model enters through the same native scalar selections and shared derivative.
fn prepare<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    full_rank: bool,
    budget: &mut Budget,
) -> TestResult<inputs::Case<P>> {
    if full_rank {
        models::prepare(fixture, contract, true, budget)
    } else {
        inputs::nominal(fixture, contract, budget)
    }
}
