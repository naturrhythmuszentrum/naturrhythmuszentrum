// SPDX-License-Identifier: AGPL-3.0-only
//! Foreign identities, missing joint coverage and every original revision refuse atomically.

use super::{reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::readings::{LcRootJacobian, ReciprocalJacobianCache, ReciprocalJacobianKey};
use nrz_scalar::{ArithmeticContext, LiteralLimits};
use std::rc::Rc;

/// Both original values and all four original radius parameters stale pending and published moments.
/// ´test:integration:lc-root-covariance-rechecks-original-versions´
#[test]
fn lc_root_covariance_rechecks_original_versions() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for change in 0..6 {
        let mut case = inputs::prepare(
            &fixture,
            &contract,
            [
                [0.25_f64, 0.015_625, 0.000_976_562_5],
                [0.0625, 0.003_906_25, 0.000_244_140_625],
            ],
            [0.007_812_5, 0.001_953_125],
            true,
            &mut budget,
        )?;
        let plan = inputs::plan(&fixture, &case, &mut budget)?;
        let pending = fixture.plan(case.original.clone(), &mut budget)?;
        let context = ArithmeticContext::new()?;
        let evaluated = case
            .heap
            .evaluate_lc_spectrum(&plan, &context, &mut budget)?;
        let publication = fixture.publication(plan.clone(), &mut budget)?;
        let published = case
            .heap
            .publish_lc_spectrum(&publication, &context, &mut budget)?;
        super::invariants::mutate(&mut case.heap, &case.original, change)?;
        let before = case.heap.provenance_usage();
        assert!(
            pending
                .with_reciprocal_statistics(
                    &case.heap,
                    case.derivative.clone(),
                    case.companion,
                    &mut budget
                )
                .is_err()
        );
        assert_eq!(case.heap.provenance_usage(), before);
        assert!(
            case.heap
                .evaluate_lc_spectrum(&plan, &context, &mut budget)
                .is_err()
        );
        assert_eq!(case.heap.provenance_usage(), before);
        assert!(evaluated.root_covariance(&case.heap, &context).is_err());
        assert_eq!(case.heap.provenance_usage(), before);
        assert!(evaluated.root_partials(&case.heap, &context).is_err());
        assert_eq!(case.heap.provenance_usage(), before);
        assert!(published.root_covariance(&case.heap, &context).is_err());
        assert_eq!(case.heap.provenance_usage(), before);
        assert!(contract.exact().native().witness().is_discharged());
    }
    Ok(())
}

/// Equal bytes and equal schemas do not authorize another selected source or heap model.
/// ´test:integration:lc-root-covariance-refuses-substituted-inputs-and-models´
#[test]
fn lc_root_covariance_refuses_substituted_inputs_and_models() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let values = [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]];
    let mut case = inputs::prepare(
        &fixture,
        &contract,
        values,
        [0.007_812_5, 0.001_953_125],
        true,
        &mut budget,
    )?;
    let foreign = inputs::prepare(
        &fixture,
        &contract,
        values,
        [0.007_812_5, 0.001_953_125],
        true,
        &mut budget,
    )?;
    let substituted = fixture.import_into(&mut case.heap, values, &mut budget)?;
    let plan = fixture.plan(case.original.clone(), &mut budget)?;
    let different = fixture.plan(substituted, &mut budget)?;
    let before = case.heap.provenance_usage();
    for (selected, derivative, companion) in [
        (different, case.derivative.clone(), case.companion),
        (plan.clone(), foreign.derivative, case.companion),
        (plan.clone(), case.derivative.clone(), foreign.companion),
    ] {
        assert!(
            selected
                .with_reciprocal_statistics(&case.heap, derivative, companion, &mut budget)
                .is_err()
        );
        assert_eq!(case.heap.provenance_usage(), before);
    }
    let admitted = plan.with_reciprocal_statistics(
        &case.heap,
        case.derivative.clone(),
        case.companion,
        &mut budget,
    )?;
    assert!(
        admitted
            .with_reciprocal_statistics(&case.heap, case.derivative, case.companion, &mut budget)
            .is_err()
    );
    assert_eq!(case.heap.provenance_usage(), before);
    Ok(())
}

/// An admitted PSD completion cannot supply an unknown cross block to root contraction.
/// ´test:integration:lc-root-covariance-requires-complete-source-coverage´
#[test]
fn lc_root_covariance_requires_complete_source_coverage() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let case = inputs::prepare(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        false,
        &mut budget,
    )?;
    let before = case.heap.provenance_usage();
    let plan = fixture.plan(case.original.clone(), &mut budget)?;
    assert!(matches!(
        plan.with_reciprocal_statistics(&case.heap, case.derivative, case.companion, &mut budget),
        Err(nrz_matrix::SpectralError::Joint(
            nrz_matrix::JointError::MissingCrossBlock
        ))
    ));
    assert_eq!(case.heap.provenance_usage(), before);
    Ok(())
}

/// Omitting a physical derivative direction cannot drop its declared stochastic variation.
/// ´test:integration:lc-root-jacobian-refuses-fixed-directions´
#[test]
fn lc_root_jacobian_refuses_fixed_directions() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let key =
        ReciprocalJacobianKey::new(graph, fixture.law.operator(), &["C".into()], &mut budget)?;
    let mut cache = ReciprocalJacobianCache::new();
    let exact = cache.get_or_derive(graph, &key, &mut budget)?;
    let contract = nrz_matrix::ReciprocalDerivativeContract::new(
        graph,
        exact,
        LiteralLimits::default(),
        &mut budget,
    )?;
    let case = inputs::prepare(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        true,
        &mut budget,
    )?;
    let before = case.heap.provenance_usage();
    assert!(inputs::plan(&fixture, &case, &mut budget).is_err());
    assert_eq!(case.heap.provenance_usage(), before);
    Ok(())
}

/// Equal operator syntax cannot substitute a differently named original physical reference.
/// ´test:integration:lc-root-jacobian-refuses-substituted-references´
#[test]
fn lc_root_jacobian_refuses_substituted_references() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let changed = super::reciprocal_refusals::different_references(&mut fixture, &mut budget)?;
    let definition = changed.reciprocal_map().ok_or("changed definition")?;
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let key = ReciprocalJacobianKey::new(graph, definition, &[], &mut budget)?;
    let exact = ReciprocalJacobianCache::new().get_or_derive(graph, &key, &mut budget)?;
    assert_eq!(definition.reuse_key(), fixture.law.operator().reuse_key());
    assert_ne!(
        definition.frame().entries(),
        fixture.law.operator().frame().entries()
    );
    assert!(matches!(
        LcRootJacobian::new(fixture.law.clone(), exact, &mut budget),
        Err(nrz_bindings::readings::SpectralLawError::Transport)
    ));
    Ok(())
}

/// A positive nominal derivative cannot establish positivity of a zero-containing physical box.
/// ´test:integration:lc-root-covariance-keeps-whole-box-domain´
#[test]
fn lc_root_covariance_keeps_whole_box_domain() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for input in 0..2 {
        let mut values = [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]];
        values[input][1] = values[input][0];
        let case = inputs::prepare(
            &fixture,
            &contract,
            values,
            [0.007_812_5, 0.001_953_125],
            true,
            &mut budget,
        )?;
        let plan = inputs::plan(&fixture, &case, &mut budget)?;
        let before = case.heap.provenance_usage();
        let result = case
            .heap
            .evaluate_lc_spectrum(&plan, &ArithmeticContext::new()?, &mut budget);
        assert!(
            matches!(result, Err(nrz_matrix::SpectralError::InputNotSeparated { input: refused }) if refused == input)
        );
        assert_eq!(case.heap.provenance_usage(), before);
    }
    Ok(())
}
