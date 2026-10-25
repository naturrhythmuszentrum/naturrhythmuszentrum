// SPDX-License-Identifier: AGPL-3.0-only
//! Separate exact preparation, numerical admission and allocation owners have literal boundaries.

use super::{reciprocal_refusals as inputs, reciprocal_statistics as transport};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{Binding, ExecutionLimits};

/// Preparation is paid once; each original numerical owner adds its own measured difference.
/// ´test:integration:lc-native-reciprocal-cumulative-work-boundary´
#[test]
fn lc_native_reciprocal_cumulative_work_boundary() -> TestResult {
    const REQUIRED: u64 = 5_552_334;
    for (work, prior, success) in [
        (REQUIRED, 0, true),
        (REQUIRED - 1, 0, false),
        (REQUIRED, 1, false),
    ] {
        let mut policy = transport::policy()?.limits().clone();
        policy.work = work;
        let mut budget = Budget::new(policy)?;
        budget.work(
            prior,
            nrz_bindings::readings::algebra::Location::new("earlier reciprocal owner"),
        )?;
        let result = pipeline(&mut budget);
        assert_eq!(result.is_ok(), success, "{result:?}");
        if success {
            assert_eq!(budget.usage().work, REQUIRED);
        }
    }
    Ok(())
}

/// Existing physical fixture and deterministic recipe construction remain separate exact owners.
fn pipeline(budget: &mut Budget) -> TestResult {
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = transport::derivative(&mut fixture, budget)?;
    assert_eq!(
        contract.exact().key().usage().after.work - contract.exact().key().usage().before.work,
        411_527
    );
    assert_eq!(
        contract.exact().usage().after.work - contract.exact().usage().before.work,
        5_128_844
    );
    assert_eq!(
        contract.usage().after.work - contract.usage().before.work,
        5_460
    );
    transport::transport(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        budget,
    )
}

/// Exact input identity checks admit only at the measured Work and retained-payload ceilings.
/// ´test:integration:lc-native-reciprocal-admission-boundaries´
#[test]
fn lc_native_reciprocal_admission_boundaries() -> TestResult {
    let mut budget = transport::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = transport::derivative(&mut fixture, &mut budget)?;
    let experiment = inputs::prepare_inputs(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        &mut budget,
    )?;
    let operation = super::operator(&fixture)?;
    for (kind, required) in [(0, 4_797), (1, 149_697), (2, 149_697)] {
        for (available, success) in [(required, true), (required - 1, false)] {
            let mut limit = super::costs::policy(kind, available, 0)?;
            let result = Binding::from_symbols(
                operation.clone(),
                experiment.selected.clone(),
                ExecutionLimits::default(),
            )
            .with_reciprocal_statistics(experiment.derivative.clone(), &mut limit);
            assert_eq!(
                result.is_ok(),
                success,
                "resource {kind}, available {available}, refusal {:?}",
                result.as_ref().err()
            );
            if success {
                assert_eq!(limit.usage().work, 4_797);
            }
        }
    }
    let mut spent = super::costs::policy(0, 4_797, 1)?;
    assert!(
        Binding::from_symbols(operation, experiment.selected, ExecutionLimits::default())
            .with_reciprocal_statistics(experiment.derivative, &mut spent)
            .is_err()
    );
    assert_eq!(experiment.heap.provenance_usage().records(), 0);
    Ok(())
}

/// Statistical inventory metadata has its own ceiling, separate from retained exact derivative payload.
/// ´test:integration:lc-native-reciprocal-statistical-inventory-boundary´
#[test]
fn lc_native_reciprocal_statistical_inventory_boundary() -> TestResult {
    let mut budget = transport::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = transport::derivative(&mut fixture, &mut budget)?;
    let mut experiment = inputs::prepare_inputs(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        &mut budget,
    )?;
    inputs::attach(&mut experiment, true)?;
    for kind in [1, 2] {
        for (available, success) in [(5_256, true), (5_255, false)] {
            let before = experiment.heap.provenance_usage();
            let binding = inputs::request(&fixture, &experiment, &mut budget)?;
            let policy = super::costs::policy(kind, available, 0)?;
            let result = binding.validate_with_resource_budget(&mut experiment.heap, &policy);
            assert_eq!(
                result.is_ok(),
                success,
                "resource {kind}, available {available}, refusal {:?}",
                result.as_ref().err()
            );
            if let Ok(bound) = result {
                assert_eq!(bound.statistical_work(), 928);
                let candidate = bound.evaluate(bound.normalized_inputs()?)?;
                let outcome = bound.publish(candidate)?;
                assert_eq!(outcome.output().len(), 1);
            }
            if success {
                // One immutable operation record and its one output evidence node.
                assert_eq!(
                    experiment.heap.provenance_usage().records(),
                    before.records() + 2
                );
            } else {
                assert_eq!(experiment.heap.provenance_usage(), before);
            }
        }
    }
    assert_eq!(experiment.heap.provenance_usage().records(), 4);
    Ok(())
}
