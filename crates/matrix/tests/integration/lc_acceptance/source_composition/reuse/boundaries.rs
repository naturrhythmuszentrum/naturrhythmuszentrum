// SPDX-License-Identifier: AGPL-3.0-only
//! Literal retained-root consumer limits keep prior Work and resident payload in the caller ledger.

use super::{Fixture, ReciprocalDerivativeContract, TestResult, ledger, mapped, shared};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both formats and ranks retain literal hit limits after the original root and binding are admitted.
/// ´test:integration:lc-source-retained-root-literal-boundaries´
#[test]
fn lc_source_retained_root_literal_boundaries() -> TestResult {
    let mut budget = shared::policy()?;
    let mut fixture = Fixture::with_mapped_sources(budget.limits().clone())?;
    let contract = shared::derivative(&mut fixture, &mut budget)?;
    for full in [false, true] {
        boundaries::<f32>(&fixture, &contract, full, [174_145, 1_944])?;
        boundaries::<f64>(&fixture, &contract, full, [175_201, 2_616])?;
    }
    Ok(())
}

/// Each allowance varies independently; a refused consumer never retires its original root.
fn boundaries<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    full: bool,
    bytes: [u64; 2],
) -> TestResult {
    let case = mapped::prepare::<P>(fixture, full, true)?;
    let binding = mapped::binding(fixture, contract, &case, [1, 5], &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let original = case
        .heap
        .evaluate_lc_spectrum(binding.spectrum(), &context, &mut ledger()?)?;
    let required = [if full { 754 } else { 742 }, bytes[0], bytes[1]];
    for (kind, threshold) in required.into_iter().enumerate() {
        for (available, prior, success) in [
            (threshold, 0, true),
            (threshold - 1, 0, false),
            (threshold, 1, kind != 0),
        ] {
            let mut budget = limited(kind, available)?;
            budget.work(prior, Location::new("prior retained-root consumer"))?;
            let before = budget.usage();
            let result = case.heap.evaluate_lc_source_statistics_reusing(
                &binding,
                &original,
                &context,
                &mut budget,
            );
            assert_eq!(
                result.is_ok(),
                success,
                "kind {kind}, limit {available}, prior {prior}: {result:?}"
            );
            assert!(budget.usage().work >= prior);
            if let Ok(value) = result {
                assert_eq!(budget.usage().work, required[0] + prior);
                assert_eq!(value.usage(), [before, budget.usage()]);
                assert_eq!(value.spectrum().usage(), original.usage());
            }
            original.normalized(&case.heap, &context)?;
        }
    }
    let mut budget = limited(1, required[1])?;
    let refused = budget.with_retained_payload(
        1,
        Location::new("resident retained-root prefix"),
        |budget| {
            case.heap
                .evaluate_lc_source_statistics_reusing(&binding, &original, &context, budget)
        },
    );
    assert!(refused.is_err());
    assert_eq!(budget.usage().work, 1);
    let before = budget.usage();
    let retried = case.heap.evaluate_lc_source_statistics_reusing(
        &binding,
        &original,
        &context,
        &mut budget,
    )?;
    assert_eq!(retried.usage(), [before, budget.usage()]);
    assert_eq!(budget.usage().work, required[0] + 1);
    mapped::oracle(retried.covariance(&case.heap, &context)?, full)?;
    Ok(())
}

/// Lower exactly one independent dimension within the unchanged default envelope.
fn limited(kind: usize, available: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        0 => limits.work = available,
        1 => limits.staged_payload_bytes = available,
        _ => limits.output_bytes = available,
    }
    Ok(Budget::new(limits)?)
}
