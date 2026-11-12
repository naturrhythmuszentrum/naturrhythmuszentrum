// SPDX-License-Identifier: AGPL-3.0-only
//! Literal caller ceilings retain consumed prefixes and atomic refusal before the connected commit.

use super::{Prepared, TestResult, inputs, prepared, prepared_with_budget, publication, shared};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Cold preparation starts after the original physical law, and shares one ledger through publication.
/// ´test:integration:lc-angular-frequency-cold-warm-work-boundaries´
#[test]
fn lc_angular_frequency_cold_warm_work_boundaries() -> TestResult {
    let prepared = prepared()?;
    pipeline_boundaries::<f32>(&prepared)?;
    pipeline_boundaries::<f64>(&prepared)
}

fn pipeline_boundaries<P: Scalar + 'static>(prepared: &Prepared) -> TestResult {
    for (cold, required) in [(true, 5_955_364), (false, 142_629)] {
        for (available, prior, success, consumed) in [
            (required, 0, true, required),
            (required - 1, 0, false, required - 1),
            (required, 1, false, required),
        ] {
            let mut budget = limited(0, available)?;
            budget.work(prior, Location::new("prior angular-frequency owner"))?;
            let result = if cold {
                prepared_with_budget(&mut budget).and_then(|fresh| run::<P>(&fresh, &mut budget))
            } else {
                run::<P>(prepared, &mut budget)
            };
            assert_eq!(
                result.is_ok(),
                success,
                "{:?} cold={cold}, available={available}, prior={prior}: {result:?}",
                P::PRECISION
            );
            assert_eq!(budget.usage().work, consumed);
        }
    }
    Ok(())
}

/// Final Work failure occurs after the numerical result; byte preflights retain their own prefixes.
/// ´test:integration:lc-angular-frequency-atomic-publication-boundaries´
#[test]
fn lc_angular_frequency_atomic_publication_boundaries() -> TestResult {
    let prepared = prepared()?;
    publication_boundaries::<f32>(&prepared, [286_780, 132_220])?;
    publication_boundaries::<f64>(&prepared, [287_052, 132_320])
}

fn publication_boundaries<P: Scalar + 'static>(prepared: &Prepared, bytes: [u64; 2]) -> TestResult {
    for (kind, required, prefix) in [(0, 1_913, 1_912), (1, bytes[0], 362), (2, bytes[1], 4)] {
        for (available, success) in [(required, true), (required - 1, false)] {
            publication_attempt::<P>(
                prepared,
                kind,
                available,
                0,
                success,
                if success { 1_913 } else { prefix },
            )?;
        }
    }
    publication_attempt::<P>(prepared, 0, 1_913, 1, false, 1_913)
}

fn publication_attempt<P: Scalar + 'static>(
    prepared: &Prepared,
    kind: usize,
    available: u64,
    prior: u64,
    success: bool,
    consumed: u64,
) -> TestResult {
    let mut setup = shared::policy()?;
    let mut case = inputs::nominal::<P>(&prepared.fixture, &prepared.derivative, &mut setup)?;
    let plan = publication(prepared, &case, &mut setup)?;
    let context = ArithmeticContext::new()?;
    let before = case.heap.provenance_usage();
    let original = case
        .original
        .iter()
        .map(|reading| case.heap.reading_entries(reading))
        .collect::<Result<Vec<_>, _>>()?;
    let mut budget = limited(kind, available)?;
    budget.work(prior, Location::new("prior angular-frequency publication"))?;
    let result = case
        .heap
        .publish_lc_angular_frequency(&plan, &context, &mut budget);
    assert_eq!(
        result.is_ok(),
        success,
        "{:?} resource={kind}, available={available}, prior={prior}: {result:?}",
        P::PRECISION
    );
    assert_eq!(budget.usage().work, consumed);
    let output = if let Ok(output) = result {
        output
    } else {
        assert_eq!(case.heap.provenance_usage(), before);
        for (reading, expected) in case.original.iter().zip(original) {
            assert_eq!(case.heap.reading_entries(reading)?, expected);
        }
        case.heap.compact()?;
        // The same symbols and heap remain available, proving that no prefix was published.
        case.heap
            .publish_lc_angular_frequency(&plan, &context, &mut shared::policy()?)?
    };
    assert_eq!(output.readings().bindings().len(), 2);
    assert_eq!(output.readings().bindings()[0].companions().len(), 3);
    assert_eq!(
        case.heap
            .reading_entries(&output.readings().bindings()[0])?,
        vec![output.value(&case.heap, &context)?.center]
    );
    output.covariance(&case.heap, &context)?;
    Ok(())
}

fn run<P: Scalar + 'static>(prepared: &Prepared, budget: &mut Budget) -> TestResult {
    let mut case = inputs::nominal::<P>(&prepared.fixture, &prepared.derivative, budget)?;
    let plan = publication(prepared, &case, budget)?;
    case.heap
        .publish_lc_angular_frequency(&plan, &ArithmeticContext::new()?, budget)?;
    Ok(())
}

fn limited(kind: usize, limit: u64) -> TestResult<Budget> {
    let defaults = shared::policy()?.limits().clone();
    let limits = match kind {
        0 => Limits {
            work: limit,
            ..defaults
        },
        1 => Limits {
            staged_payload_bytes: limit,
            ..defaults
        },
        _ => Limits {
            output_bytes: limit,
            ..defaults
        },
    };
    Ok(Budget::new(limits)?)
}
