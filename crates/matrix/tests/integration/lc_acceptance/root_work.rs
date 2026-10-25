// SPDX-License-Identifier: AGPL-3.0-only
//! Literal cumulative Work boundaries retain the shared cache and atomic numerical owners.

use super::{reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::{
    Budget, Limits,
    readings::{LcRootJacobian, ReciprocalJacobianCache, ReciprocalJacobianKey, algebra::Location},
};
use nrz_matrix::ReciprocalDerivativeContract;
use nrz_scalar::{ArithmeticContext, LiteralLimits};
use std::rc::Rc;

/// One miss, one authenticated hit and one root adapter share a literal cumulative caller ledger.
/// ´test:integration:lc-root-preparation-cumulative-work-boundary´
#[test]
fn lc_root_preparation_cumulative_work_boundary() -> TestResult {
    const REQUIRED: u64 = 5_559_713;
    for (available, prior, success) in [
        (REQUIRED, 0, true),
        (REQUIRED - 1, 0, false),
        (REQUIRED, 1, false),
    ] {
        let mut limits = shared::policy()?.limits().clone();
        limits.work = available;
        let mut budget = Budget::new(limits)?;
        budget.work(prior, Location::new("earlier root preparation"))?;
        let result = preparation(&mut budget);
        assert_eq!(result.is_ok(), success, "{result:?}");
        if success {
            assert_eq!(budget.usage().work, REQUIRED);
        }
    }
    Ok(())
}

/// Exact evidence is prepared once and compatible root admission never derives another witness.
fn preparation(budget: &mut Budget) -> TestResult {
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let key = ReciprocalJacobianKey::new(graph, fixture.law.operator(), &[], budget)?;
    assert_eq!(key.usage().after.work - key.usage().before.work, 411_527);
    let mut cache = ReciprocalJacobianCache::new();
    let exact = cache.get_or_derive(graph, &key, budget)?;
    assert_eq!(
        exact.usage().after.work - exact.usage().before.work,
        5_128_844
    );
    let before = budget.usage().work;
    let cached = cache.get_or_derive(graph, &key, budget)?;
    assert_eq!(budget.usage().work - before, 9085);
    assert!(std::ptr::eq(exact.native(), cached.native()));
    let contract =
        ReciprocalDerivativeContract::new(graph, exact, LiteralLimits::default(), budget)?;
    assert_eq!(
        contract.usage().after.work - contract.usage().before.work,
        5460
    );
    let retained = cache.retained_payload_bytes();
    let result = LcRootJacobian::new(fixture.law, contract.exact().clone(), budget);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.retained_payload_bytes(), retained);
    assert_eq!(retained, 142_043);
    let root = result?;
    assert_eq!(root.usage().after.work - root.usage().before.work, 4797);
    assert!(std::ptr::eq(
        root.operator().native(),
        contract.exact().native()
    ));
    Ok(())
}

/// Exact root identity admission pays only its own comparisons and shared-record allocation.
/// ´test:integration:lc-root-adapter-resource-boundaries´
#[test]
fn lc_root_adapter_resource_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for (kind, required) in [(0, 4797), (1, 632), (2, 632)] {
        for (available, success) in [(required, true), (required - 1, false)] {
            let result = LcRootJacobian::new(
                fixture.law.clone(),
                contract.exact().clone(),
                &mut policy(kind, available, 0)?,
            );
            assert_eq!(result.is_ok(), success, "{result:?}");
        }
    }
    Ok(())
}

/// Root admission and contraction keep earlier caller Work and leave heap usage unchanged on refusal.
/// ´test:integration:lc-root-numerical-work-boundaries´
#[test]
fn lc_root_numerical_work_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    let case = inputs::prepare(
        &fixture,
        &contract,
        [[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        [0.007_812_5, 0.001_953_125],
        true,
        &mut budget,
    )?;
    let deterministic = fixture.plan(case.original.clone(), &mut budget)?;
    let context = ArithmeticContext::new()?;
    for (available, prior, success) in [(4805, 0, true), (4804, 0, false), (4805, 1, false)] {
        let before = case.heap.provenance_usage();
        let result = deterministic.clone().with_reciprocal_statistics(
            &case.heap,
            case.derivative.clone(),
            case.companion,
            &mut policy(0, available, prior)?,
        );
        assert_eq!(result.is_ok(), success, "{result:?}");
        assert_eq!(case.heap.provenance_usage(), before);
    }
    let plan = inputs::plan(&fixture, &case, &mut budget)?;
    for (available, prior, success) in [(430, 0, true), (429, 0, false), (430, 1, false)] {
        let before = case.heap.provenance_usage();
        let result =
            case.heap
                .evaluate_lc_spectrum(&plan, &context, &mut policy(0, available, prior)?);
        assert_eq!(result.is_ok(), success, "{result:?}");
        assert_eq!(case.heap.provenance_usage(), before);
    }
    Ok(())
}

/// Only the requested owner ceiling changes; production defaults and earlier usage remain intact.
pub(super) fn policy(kind: usize, available: u64, prior: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        0 => limits.work = available,
        1 => limits.staged_payload_bytes = available,
        _ => limits.output_bytes = available,
    }
    let mut budget = Budget::new(limits)?;
    budget.work(prior, Location::new("earlier root owner"))?;
    Ok(budget)
}

/// One caller ledger covers exact preparation through atomic publication without resetting Work.
/// ´test:integration:lc-root-publication-cumulative-work-boundary´
#[test]
fn lc_root_publication_cumulative_work_boundary() -> TestResult {
    const REQUIRED: u64 = 5_689_088;
    for (available, prior, success) in [
        (REQUIRED, 0, true),
        (REQUIRED - 1, 0, false),
        (REQUIRED, 1, false),
    ] {
        let mut limits = shared::policy()?.limits().clone();
        limits.work = available;
        let mut budget = Budget::new(limits)?;
        budget.work(prior, Location::new("earlier complete root lifecycle"))?;
        let mut fixture = Fixture::with_output_store_limits(budget.limits().clone())?;
        let contract = shared::derivative(&mut fixture, &mut budget)?;
        let mut case = inputs::nominal::<f64>(&fixture, &contract, &mut budget)?;
        let spectrum = inputs::plan(&fixture, &case, &mut budget)?;
        let publication = fixture.publication(spectrum, &mut budget)?;
        let context = ArithmeticContext::new()?;
        let before = case.heap.provenance_usage();
        let snapshot = [
            case.heap.reading_entries(&case.original[0])?,
            case.heap.reading_entries(&case.original[1])?,
        ];
        let result = case
            .heap
            .publish_lc_spectrum(&publication, &context, &mut budget);
        assert_eq!(result.is_ok(), success, "{result:?}");
        if let Ok(published) = result {
            assert_eq!(budget.usage().work, REQUIRED);
            assert!(published.root_covariance(&case.heap, &context)?.is_some());
        } else {
            assert_eq!(case.heap.provenance_usage(), before);
            for (source, expected) in case.original.iter().zip(snapshot) {
                let current = case.heap.reading_entries(source)?;
                assert_eq!(current[0].to_bits(), expected[0].to_bits());
            }
        }
    }
    Ok(())
}
