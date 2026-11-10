// SPDX-License-Identifier: AGPL-3.0-only
//! New adapter owners have separate literal limits from the unchanged native resource pins.

use super::{super::reciprocal_statistics as shared, mapped};
use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_matrix::{Heap, HeapLimits, ReciprocalDerivativeContract};
use nrz_scalar::{ArithmeticContext, Scalar};

/// One retained admission owner is checked independently against staged and output ceilings.
/// ´test:integration:lc-source-composition-literal-owner-boundaries´
#[test]
fn lc_source_composition_literal_owner_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_mapped_sources(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    boundaries::<f32>(&fixture, &contract, [173_497, 1_944])?;
    boundaries::<f64>(&fixture, &contract, [174_521, 2_616])
}

/// Every budget dimension varies alone; consumed Work is never refunded after refusal.
fn boundaries<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    evaluation_bytes: [u64; 2],
) -> TestResult {
    for full in [false, true] {
        let case = mapped::prepare::<P>(fixture, full, true)?;
        let binding = mapped::binding(fixture, contract, &case, [1, 5], &mut ledger()?)?;
        assert_eq!(binding.retained_payload_bytes(), 171_157);
        let context = ArithmeticContext::new()?;
        for (stage, required) in [
            (false, [5_551, 171_157, 171_157]),
            (
                true,
                [
                    if full { 1_099 } else { 1_087 },
                    evaluation_bytes[0],
                    evaluation_bytes[1],
                ],
            ),
        ] {
            for (kind, required_limit) in required.into_iter().enumerate() {
                for (available, prior, success) in [
                    (required_limit, 0, true),
                    (required_limit - 1, 0, false),
                    (required_limit, 1, kind != 0),
                ] {
                    let mut budget = limited(kind, available)?;
                    budget.work(prior, Location::new("earlier LC source owner"))?;
                    let result = if stage {
                        case.heap
                            .evaluate_lc_source_statistics(&binding, &context, &mut budget)
                            .map(|_| ())
                            .map_err(Into::into)
                    } else {
                        mapped::binding(fixture, contract, &case, [1, 5], &mut budget).map(|_| ())
                    };
                    assert_eq!(
                        result.is_ok(),
                        success,
                        "stage {stage}, kind {kind}, limit {available}, prior {prior}: {result:?}"
                    );
                    assert!(budget.usage().work >= prior);
                    if success {
                        assert_eq!(budget.usage().work, required[0] + prior);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Select exactly one owner dimension, leaving every other default untouched.
fn limited(kind: usize, available: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        0 => limits.work = available,
        1 => limits.staged_payload_bytes = available,
        _ => limits.output_bytes = available,
    }
    Ok(Budget::new(limits)?)
}

/// Measure the newly admitted copy import under the complete caller ledger.
/// ´test:integration:lc-source-composition-copy-import-boundaries´
#[test]
fn lc_source_composition_copy_import_boundaries() -> TestResult {
    let budget = shared::policy()?;
    let fixture = Fixture::with_mapped_sources(budget.limits().clone())?;
    import::<f32>(&fixture)?;
    import::<f64>(&fixture)
}

/// The exact-copy import adds scratch accounting; native heap storage keeps its separate limits.
fn import<P: Scalar + 'static>(fixture: &Fixture) -> TestResult {
    let regions = mapped::regions::<P>()?;
    for (kind, required) in [(0, 170_620), (1, 2_000)] {
        for (available, prior, success) in [
            (required, 0, true),
            (required - 1, 0, false),
            (required, 1, kind != 0),
        ] {
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut budget = limited(kind, available)?;
            budget.work(prior, Location::new("earlier exact-copy import"))?;
            let result = mapped::import_with(fixture, &mut heap, &regions, &mut budget);
            assert_eq!(
                result.is_ok(),
                success,
                "kind {kind}, limit {available}, prior {prior}: {result:?}"
            );
            assert!(budget.usage().work >= prior);
            if success {
                assert_eq!(budget.usage().work, 170_620 + prior);
            } else {
                mapped::import_with(fixture, &mut heap, &regions, &mut ledger()?)?;
            }
        }
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = limited(1, 2_000)?;
    let refused =
        budget.with_retained_payload(1, Location::new("retained import prefix"), |budget| {
            mapped::import_with(fixture, &mut heap, &regions, budget)
        });
    assert!(refused.is_err());
    // No new output payload is owned by the exact-copy check itself.
    mapped::import_with(fixture, &mut heap, &regions, &mut limited(2, 0)?)?;
    Ok(())
}
