// SPDX-License-Identifier: AGPL-3.0-only
//! Original L/C source Jacobians transport a declared common-source local linear model.

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_bindings::{
    Budget, Limits,
    readings::{ReciprocalJacobianCache, ReciprocalJacobianKey},
};
use nrz_matrix::{
    Binding, CompanionSetDraft, ExecutionLimits, JointLimits, MomentAssurance,
    ReciprocalDerivativeContract,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};
use std::rc::Rc;

/// Every covariance entry follows the independently calculated signed common-source derivative.
/// ´test:integration:lc-native-reciprocal-transports-original-covariance´
#[test]
fn lc_native_reciprocal_transports_original_covariance() -> TestResult {
    let mut budget = policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())
        .map_err(|error| format!("fixture store construction: {error:?}"))?;
    let contract = derivative(&mut fixture, &mut budget)?;
    transport(
        &fixture,
        &contract,
        [[0.25_f32, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        &mut budget,
    )?;
    transport(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        &mut budget,
    )
}

/// Exact original preparation shares a caller ledger and its retained derivative cache.
pub(super) fn derivative(
    fixture: &mut Fixture,
    budget: &mut Budget,
) -> TestResult<ReciprocalDerivativeContract> {
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("unshared original graph")?;
    let key = ReciprocalJacobianKey::new(graph, fixture.law.operator(), &[], budget).map_err(
        |error| {
            format!(
                "Jacobian key, usage {:?}, limits {:?}: {error:?}",
                budget.usage(),
                budget.limits()
            )
        },
    )?;
    let mut cache = ReciprocalJacobianCache::new();
    let exact = cache.get_or_derive(graph, &key, budget).map_err(|error| {
        format!(
            "Jacobian derivation, usage {:?}, limits {:?}: {error:?}",
            budget.usage(),
            budget.limits()
        )
    })?;
    assert_eq!(cache.len(), 1);
    Ok(
        ReciprocalDerivativeContract::new(graph, exact, LiteralLimits::default(), budget).map_err(
            |error| {
                format!(
                    "Jacobian lowering, usage {:?}, limits {:?}: {error:?}",
                    budget.usage(),
                    budget.limits()
                )
            },
        )?,
    )
}

/// The existing explicit LC fixture policy pays for shared exact preparation and all numerical uses.
pub(super) fn policy() -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work: 16_000_000,
        replay_work: 16_000_000,
        ..Limits::default()
    })?)
}

/// Native execution retains the original model, exact Jacobian and deterministic interval scope.
pub(super) fn transport<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    values: [[P; 3]; 2],
    coefficients: [P; 2],
    budget: &mut Budget,
) -> TestResult {
    let operation = super::operator(fixture)?;
    let (mut heap, original) = fixture.import_with_budget(values, budget)?;
    let selected = original
        .iter()
        .map(|source| heap.select_reading(source, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let handles = super::selected_joint::capture_handles(&mut heap, &selected)?;
    let model = super::selected_joint::joint_model(&selected, &coefficients)?;
    let identity = model.id();
    let sources = model
        .sources()
        .iter()
        .map(nrz_matrix::Source::id)
        .collect::<Vec<_>>();
    let draft = CompanionSetDraft::new(
        model,
        super::selected_joint::capture_targets(&heap, handles)?,
        [vec![0], vec![1]],
    );
    heap.publish_joint(draft, JointLimits::default())?;
    let context = ArithmeticContext::new()?;
    let evaluation = heap.evaluate_reciprocal_derivative(contract, &original, &context, budget)?;
    let binding = Binding::from_symbols(operation, selected, ExecutionLimits::default())
        .with_reciprocal_statistics(evaluation, budget)?;
    let bound = binding.validate_with_resource_budget(&mut heap, budget)?;
    assert_eq!(bound.statistical_work(), 928);
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = crate::kernel_testing::output(&outcome)?;
    verify(&heap, output, contract, identity, &sources)
}

/// Historical exact proof and numerical moments retain their original meanings independently.
fn verify<P: Scalar + 'static>(
    heap: &nrz_matrix::Heap,
    output: nrz_matrix::PublishedBundle<P>,
    contract: &ReciprocalDerivativeContract,
    identity: nrz_matrix::ModelId,
    sources: &[nrz_matrix::SourceId],
) -> TestResult {
    let moments = heap.moments(heap.moment_companion(output.value())?)?;
    assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
    assert_eq!(moments.model(), identity);
    assert_eq!(moments.source_ids(), sources);
    assert!(moments.means().is_none());
    let retained = moments
        .allocation_inventory()
        .operation()
        .ok_or("original operation")?;
    assert!(retained.carries_statistics());
    assert!(std::ptr::eq(
        retained
            .reciprocal_jacobian()
            .ok_or("original Jacobian")?
            .native(),
        contract.exact().native()
    ));
    let inventory = moments.allocation_inventory();
    let width = std::mem::size_of::<P>();
    assert_eq!(inventory.peak_payload_bytes(), 172 * width);
    assert_eq!(inventory.metadata_bytes(), 5_256);
    assert_eq!(inventory.backend_allowance_bytes(), 928 * width);
    covariance(moments.covariance())?;
    for (index, expected) in [(1, Rational::new(-1, 10)?), (2, Rational::new(1, 6250)?)] {
        let (center, _, error) = crate::kernel_testing::entry(heap, output, index)?;
        assert!(expected.covered(center, error)?);
    }
    for (index, denominator) in [(1, 75), (1, 85), (2, 46_875), (2, 53_125)] {
        let numerator = if index == 1 { -8 } else { 8 };
        let (center, total, _) = crate::kernel_testing::entry(heap, output, index)?;
        assert!(Rational::new(numerator, denominator)?.covered(center, total)?);
    }
    assert_eq!(
        heap.value(output.value())?.coverage(),
        nrz_matrix::Coverage::DeclaredBounds
    );
    Ok(())
}

/// J is (0, 1/320, -1/200000, 0) for the one declared rank-one fluctuation.
fn covariance<P: Scalar + 'static>(values: &nrz_matrix::ParameterView<'_, P>) -> TestResult {
    for column in 0..4 {
        for row in 0..4 {
            let (numerator, denominator) = match (row, column) {
                (1, 1) => (1, 102_400),
                (1, 2) | (2, 1) => (-1, 64_000_000),
                (2, 2) => (1, 40_000_000_000),
                _ => (0, 1),
            };
            assert!(Rational::new(numerator, denominator)?.covered(
                values.get(row, column)?.to_f64(),
                values.error(row, column)?.to_f64()
            )?);
        }
    }
    Ok(())
}
