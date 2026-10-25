// SPDX-License-Identifier: AGPL-3.0-only
//! The supplementary full-rank model retains exact native phase and metadata boundaries.

use super::{
    family_models as models, reciprocal_statistics as shared, root_support as inputs,
    root_work::policy,
};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{Binding, ExecutionLimits, ReciprocalDerivativeContract};
use nrz_scalar::Scalar;

/// Native statistical Work, scratch slots and metadata retain their own success/refusal ceilings.
/// ´test:integration:lc-operator-root-family-full-rank-native-boundaries´
#[test]
fn lc_operator_root_family_full_rank_native_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    boundaries::<f32>(&fixture, &contract, 3840, &mut budget)?;
    boundaries::<f64>(&fixture, &contract, 7680, &mut budget)
}

fn boundaries<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    backend: usize,
    budget: &mut Budget,
) -> TestResult {
    let mut case = models::prepare::<P>(fixture, contract, true, budget)?;
    for (kind, required) in [(0, 960), (1, 960), (2, 5256), (3, 5256)] {
        for (available, success) in [(required, true), (required - 1, false)] {
            attempt(fixture, &mut case, kind, available, success, budget)?;
        }
    }
    let output = super::family_support::operator(fixture, &mut case, budget)?;
    let moments = case
        .heap
        .moments(case.heap.moment_companion(output.value())?)?;
    assert_eq!(
        moments.allocation_inventory().backend_allowance_bytes(),
        backend
    );
    assert_eq!(moments.allocation_inventory().metadata_bytes(), 5256);
    assert_eq!(
        moments.allocation_inventory().peak_payload_bytes(),
        172 * size_of::<P>()
    );
    Ok(())
}

fn attempt<P: Scalar + 'static>(
    fixture: &Fixture,
    case: &mut inputs::Case<P>,
    kind: usize,
    available: usize,
    success: bool,
    budget: &mut Budget,
) -> TestResult {
    let limits = match kind {
        0 => ExecutionLimits::new(4096, available, 1_048_576),
        1 => ExecutionLimits::new(4096, 1_048_576, available),
        _ => ExecutionLimits::default(),
    };
    let resource = if kind >= 2 {
        policy(kind - 1, u64::try_from(available)?, 0)?
    } else {
        shared::policy()?
    };
    let binding = Binding::from_symbols(super::operator(fixture)?, case.selected.clone(), limits)
        .with_reciprocal_statistics(case.derivative.clone(), budget)?;
    let before = case.heap.provenance_usage();
    let result = binding.validate_with_resource_budget(&mut case.heap, &resource);
    assert_eq!(
        result.is_ok(),
        success,
        "owner {kind}: {:?}",
        result.as_ref().err()
    );
    if let Ok(bound) = result {
        assert_eq!(bound.statistical_work(), 960);
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        bound.publish(candidate)?;
    } else {
        assert_eq!(case.heap.provenance_usage(), before);
    }
    Ok(())
}
