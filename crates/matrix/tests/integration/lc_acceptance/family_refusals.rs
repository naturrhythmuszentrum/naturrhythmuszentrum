// SPDX-License-Identifier: AGPL-3.0-only
//! Individually valid outputs cannot substitute another original model, target or reference.

use super::{family_support as family, reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::{
    Budget,
    readings::{ReciprocalJacobianCache, ReciprocalJacobianKey},
};
use nrz_matrix::{
    Binding, CompanionSetDraft, ExecutionLimits, Heap, JointLimits, JointModel, MomentRef,
    PublishedLcSpectrum, ReciprocalDerivativeContract,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits};
use std::rc::Rc;

/// Equal-valued outputs from another model or another target pair refuse even in the same heap.
/// ´test:integration:lc-operator-root-family-refuses-model-and-source-substitution´
#[test]
fn lc_operator_root_family_refuses_model_and_source_substitution() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for same_model in [false, true] {
        let mut case = inputs::nominal::<f64>(&fixture, &contract, &mut budget)?;
        let plan = inputs::plan(&fixture, &case, &mut budget)?;
        let model = same_model
            .then(|| {
                case.heap
                    .statistical(case.companion)
                    .map(|view| view.model().clone())
            })
            .transpose()?;
        substitute(&fixture, &contract, &mut case, model, &mut budget)?;
        let other = family::operator(&fixture, &mut case, &mut budget)?;
        let moments = case.heap.moment_companion(other.value())?;
        let publication = fixture.publication(plan, &mut budget)?;
        let roots =
            case.heap
                .publish_lc_spectrum(&publication, &ArithmeticContext::new()?, &mut budget)?;
        assert_eq!(
            case.heap.moments(moments)?.model()
                == roots
                    .evaluation()
                    .plan()
                    .statistical_identity()
                    .ok_or("root identity")?
                    .0,
            same_model
        );
        assert!(
            roots
                .root_covariance(&case.heap, &ArithmeticContext::new()?)?
                .is_some()
        );
        let before = case.heap.provenance_usage();
        let result = case.heap.lc_operator_root_covariance(
            moments,
            &roots,
            &ArithmeticContext::new()?,
            &mut budget,
        );
        assert!(
            matches!(&result, Err(nrz_matrix::SpectralError::Identity)),
            "{result:?}"
        );
        assert_eq!(case.heap.provenance_usage(), before);
    }
    Ok(())
}

/// Reusing source IDs on different physical target occupations cannot replace the original experiment.
fn substitute(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    case: &mut inputs::Case<f64>,
    model: Option<JointModel<f64>>,
    budget: &mut Budget,
) -> TestResult {
    case.original = fixture.import_into(
        &mut case.heap,
        [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        budget,
    )?;
    case.selected = case
        .original
        .iter()
        .map(|input| case.heap.select_reading(input, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let handles = super::selected_joint::capture_handles(&mut case.heap, &case.selected)?;
    let model = model.map_or_else(
        || super::selected_joint::joint_model(&case.selected, &[0.007_812_5, 0.001_953_125]),
        Ok,
    )?;
    let targets = super::selected_joint::capture_targets(&case.heap, handles)?;
    case.companion = case.heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0], vec![1]]),
        JointLimits::default(),
    )?;
    case.derivative = case.heap.evaluate_reciprocal_derivative(
        contract,
        &case.original,
        &ArithmeticContext::new()?,
        budget,
    )?;
    Ok(())
}

/// Original derivative and native admission refuse changed reference identities before any output.
/// ´test:integration:lc-operator-root-family-refuses-reference-substitution´
#[test]
fn lc_operator_root_family_refuses_reference_substitution() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let changed = super::reciprocal_refusals::different_references(&mut fixture, &mut budget)?;
    let definition = changed.reciprocal_map().ok_or("changed map")?;
    assert_eq!(definition.reuse_key(), fixture.law.operator().reuse_key());
    assert_ne!(
        definition.frame().entries(),
        fixture.law.operator().frame().entries()
    );
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let key = ReciprocalJacobianKey::new(graph, definition, &[], &mut budget)?;
    let exact = ReciprocalJacobianCache::new().get_or_derive(graph, &key, &mut budget)?;
    let altered =
        ReciprocalDerivativeContract::new(graph, exact, LiteralLimits::default(), &mut budget)?;
    let original = shared::derivative(&mut fixture, &mut budget)?;
    let mut case = inputs::nominal::<f64>(&fixture, &original, &mut budget)?;
    let outputs = family::publish(&fixture, &mut case, &mut budget)?;
    let before = case.heap.provenance_usage();
    let result = case.heap.evaluate_reciprocal_derivative(
        &altered,
        &case.original,
        &ArithmeticContext::new()?,
        &mut budget,
    );
    assert!(
        matches!(
            &result,
            Err(nrz_matrix::ReciprocalDerivativeError::Reading(
                nrz_bindings::readings::ReadingError::Reference
            ))
        ),
        "{result:?}"
    );
    assert_eq!(case.heap.provenance_usage(), before);
    let result = Binding::from_symbols(changed, case.selected.clone(), ExecutionLimits::default())
        .with_reciprocal_statistics(case.derivative.clone(), &mut budget);
    assert!(matches!(
        result,
        Err(nrz_matrix::ReciprocalDerivativeError::Identity)
    ));
    assert_eq!(case.heap.provenance_usage(), before);
    case.heap.lc_operator_root_covariance(
        outputs.moments,
        &outputs.roots,
        &ArithmeticContext::new()?,
        &mut budget,
    )?;
    Ok(())
}

/// A foreign opaque companion cannot be resolved by a coincident record index.
/// ´test:integration:lc-operator-root-family-refuses-foreign-moments´
#[test]
fn lc_operator_root_family_refuses_foreign_moments() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let mut case = inputs::nominal::<f64>(&fixture, &contract, &mut budget)?;
    let outputs = family::publish(&fixture, &mut case, &mut budget)?;
    let mut foreign = inputs::nominal::<f64>(&fixture, &contract, &mut budget)?;
    let imported = family::publish(&fixture, &mut foreign, &mut budget)?;
    refuse(&case.heap, imported.moments, &outputs.roots, &mut budget)?;
    refuse(&case.heap, outputs.moments, &imported.roots, &mut budget)
}

/// A deterministic root publication cannot silently acquire moments from a statistical operator.
/// ´test:integration:lc-operator-root-family-requires-both-statistical-outputs´
#[test]
fn lc_operator_root_family_requires_both_statistical_outputs() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let mut case = inputs::nominal::<f64>(&fixture, &contract, &mut budget)?;
    let output = family::operator(&fixture, &mut case, &mut budget)?;
    let moments = case.heap.moment_companion(output.value())?;
    let deterministic = fixture.plan(case.original.clone(), &mut budget)?;
    let publication = fixture.publication(deterministic, &mut budget)?;
    let roots =
        case.heap
            .publish_lc_spectrum(&publication, &ArithmeticContext::new()?, &mut budget)?;
    assert!(roots.evaluation().moment_assurance().is_none());
    refuse(&case.heap, moments, &roots, &mut budget)
}

pub(super) fn refuse<P: nrz_scalar::Scalar + 'static>(
    heap: &Heap,
    moments: MomentRef<P>,
    roots: &PublishedLcSpectrum<P>,
    budget: &mut Budget,
) -> TestResult {
    let before = heap.provenance_usage();
    assert!(
        heap.lc_operator_root_covariance(moments, roots, &ArithmeticContext::new()?, budget)
            .is_err()
    );
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}
