// SPDX-License-Identifier: AGPL-3.0-only
//! Each mean publication has exact caller-owned Work and payload boundaries.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_matrix::{DependencyPolicy, Heap, PublishedPhasorSolve, ReadingBinding, SymbolState};
use nrz_scalar::ArithmeticContext;

/// Every positive ceiling admits at N and refuses atomically at N-1 with either dependency policy.
/// ´test:integration:driven-mean-publication-owner-boundaries´
#[test]
fn driven_mean_publication_owner_boundaries() -> TestResult {
    for dependencies in [DependencyPolicy::Entries, DependencyPolicy::WholeBase] {
        for (output, profile) in [[8541, 801_329, 711_328], [8551, 801_395, 711_352]]
            .into_iter()
            .enumerate()
        {
            boundary(dependencies, output, profile)?;
        }
    }
    Ok(())
}

fn boundary(dependencies: DependencyPolicy, output: usize, profile: [u64; 3]) -> TestResult {
    let mut case = fixture::lifecycle_case(dependencies, &mut super::budget()?)?;
    let context = ArithmeticContext::new()?;
    let previous = if output == 1 {
        Some(
            case.heap
                .publish_phasor_solve(&case.publications[0], &context, &mut ledger()?)?,
        )
    } else {
        None
    };
    let before = snapshot(&case, previous.as_ref())?;
    for kind in 0..3 {
        let mut limits = profile;
        limits[kind] -= 1;
        assert!(
            case.heap
                .publish_phasor_solve(
                    &case.publications[output],
                    &context,
                    &mut policy(limits, 0)?,
                )
                .is_err()
        );
        assert_eq!(snapshot(&case, previous.as_ref())?, before);
    }
    assert!(
        case.heap
            .publish_phasor_solve(
                &case.publications[output],
                &context,
                &mut policy(profile, 1)?,
            )
            .is_err()
    );
    assert_eq!(snapshot(&case, previous.as_ref())?, before);
    let mut resources = policy(profile, 0)?;
    let published =
        case.heap
            .publish_phasor_solve(&case.publications[output], &context, &mut resources)?;
    assert_eq!(resources.usage().work, profile[0]);
    assert_eq!(snapshot(&case, previous.as_ref())?, before);
    assert_eq!(published.readings().bindings().len(), 6);
    for reading in published.readings().bindings() {
        assert_eq!(reading.state(), SymbolState::Published);
        reading_bits(&case.heap, reading)?;
    }
    Ok(())
}

fn policy(profile: [u64; 3], prior: u64) -> TestResult<Budget> {
    let mut resources = Budget::new(Limits {
        work: profile[0],
        staged_payload_bytes: profile[1],
        output_bytes: profile[2],
        ..Limits::default()
    })?;
    resources.work(prior, Location::new("prior driven publication"))?;
    Ok(resources)
}

/// Public observations retain all source and earlier output values, radii and live states.
fn snapshot(
    case: &fixture::Case<f64>,
    previous: Option<&PublishedPhasorSolve<f64>>,
) -> TestResult<Vec<(SymbolState, Vec<u64>)>> {
    case.bindings
        .iter()
        .chain(
            previous
                .into_iter()
                .flat_map(|mean| mean.readings().bindings()),
        )
        .map(|reading| Ok((reading.state(), reading_bits(&case.heap, reading)?)))
        .collect()
}

fn reading_bits(heap: &Heap, reading: &ReadingBinding<f64>) -> TestResult<Vec<u64>> {
    let mut values = heap.reading_entries(reading)?;
    for (index, companion) in reading.companions().iter().enumerate() {
        if companion
            .targets()
            .iter()
            .any(|target| target.reading() == reading.reading())
        {
            values.extend(heap.reading_companion_entries(reading, index)?);
        }
    }
    Ok(values.into_iter().map(f64::to_bits).collect())
}
