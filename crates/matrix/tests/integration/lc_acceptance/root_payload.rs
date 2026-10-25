// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical allocations have literal boundaries while deterministic resource pins remain fixed.

use super::{reciprocal_statistics as shared, root_support as inputs, root_work::policy};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::{Budget, readings::algebra::Location};
use nrz_matrix::ReciprocalDerivativeContract;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Original statistical input and evaluated output have separate exact staged/output ceilings.
/// ´test:integration:lc-root-covariance-payload-boundaries´
#[test]
fn lc_root_covariance_payload_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    numerical::<f32>(
        &fixture,
        &contract,
        [[152_537, 152_537], [155_217, 1448]],
        &mut budget,
    )?;
    numerical::<f64>(
        &fixture,
        &contract,
        [[152_561, 152_561], [155_489, 1504]],
        &mut budget,
    )
}

/// Earlier retained caller payload consumes the same staged allowance on every numerical attempt.
fn numerical<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    required: [[u64; 2]; 2],
    budget: &mut Budget,
) -> TestResult {
    let case = inputs::nominal::<P>(fixture, contract, budget)?;
    let deterministic = fixture.plan(case.original.clone(), budget)?;
    let plan = inputs::plan(fixture, &case, budget)?;
    let context = ArithmeticContext::new()?;
    for (owner, requirements) in required.into_iter().enumerate() {
        for (kind, need) in [1, 2].into_iter().zip(requirements) {
            for (available, success) in [(need, true), (need - 1, false)] {
                let before = case.heap.provenance_usage();
                let mut limits = policy(kind, available, 0)?;
                let result = if owner == 0 {
                    deterministic
                        .clone()
                        .with_reciprocal_statistics(
                            &case.heap,
                            case.derivative.clone(),
                            case.companion,
                            &mut limits,
                        )
                        .map(|_| ())
                } else {
                    case.heap
                        .evaluate_lc_spectrum(&plan, &context, &mut limits)
                        .map(|_| ())
                };
                assert_eq!(
                    result.is_ok(),
                    success,
                    "owner {owner}, kind {kind}: {result:?}"
                );
                assert_eq!(case.heap.provenance_usage(), before);
            }
        }
    }
    let before = case.heap.provenance_usage();
    let result = policy(1, required[1][0], 0)?.with_retained_payload(
        1,
        Location::new("earlier retained root owner"),
        |limits| case.heap.evaluate_lc_spectrum(&plan, &context, limits),
    );
    assert!(result.is_err());
    assert_eq!(case.heap.provenance_usage(), before);
    Ok(())
}

/// Both formats publish every root and covariance block together at the exact owner ceilings.
/// ´test:integration:lc-root-covariance-publication-boundaries´
#[test]
fn lc_root_covariance_publication_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    publication::<f32>(&fixture, &contract, [5238, 1_010_924, 506_116])?;
    publication::<f64>(&fixture, &contract, [5238, 1_011_032, 506_224])
}

/// Failed publications preserve complete original value and provenance usage on the same heap.
fn publication<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    requirements: [u64; 3],
) -> TestResult {
    for (kind, required) in requirements.into_iter().enumerate() {
        for (available, success) in [(required, true), (required - 1, false)] {
            let mut setup = shared::policy()?;
            let mut case = inputs::nominal::<P>(fixture, contract, &mut setup)?;
            let spectrum = inputs::plan(fixture, &case, &mut setup)?;
            let plan = fixture.publication(spectrum, &mut setup)?;
            let context = ArithmeticContext::new()?;
            let before = case.heap.provenance_usage();
            let original = values(&case)?;
            let result =
                case.heap
                    .publish_lc_spectrum(&plan, &context, &mut policy(kind, available, 0)?);
            assert_eq!(result.is_ok(), success, "kind {kind}: {result:?}");
            if let Ok(published) = result {
                assert_eq!(published.readings().bindings().len(), 6);
                assert!(published.root_covariance(&case.heap, &context)?.is_some());
            } else {
                assert_eq!(case.heap.provenance_usage(), before);
                assert_eq!(values(&case)?, original);
            }
        }
    }
    Ok(())
}

/// Exact represented source bits remain available after every refused publication.
fn values<P: Scalar + 'static>(case: &inputs::Case<P>) -> TestResult<Vec<u64>> {
    case.original
        .iter()
        .map(|input| Ok(case.heap.reading_entries(input)?[0].to_f64().to_bits()))
        .collect()
}
