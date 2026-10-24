// SPDX-License-Identifier: AGPL-3.0-only
//! Complete published driven families give write owners exact refusal and atomicity boundaries.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{FaultKind, Location},
};
use nrz_matrix::{DependencyPolicy, LiveBindingError};
use nrz_scalar::ArithmeticContext;

/// Exact ceilings admit one write; one-below and prior Work refuse on the same unchanged heap.
/// ´test:integration:driven-lifecycle-write-owner-boundaries´
#[test]
fn driven_lifecycle_write_owner_boundaries() -> TestResult {
    // Fine writes add forty charged source-selection comparisons across the two solve closures.
    for (dependencies, profiles) in [
        (
            DependencyPolicy::Entries,
            [[2312, 17, 8], [2797, 17, 8], [37, 32, 0]],
        ),
        (
            DependencyPolicy::WholeBase,
            [[32, 17, 0], [32, 17, 0], [37, 32, 0]],
        ),
    ] {
        for (changed, profile) in [15, 16, 10].into_iter().zip(profiles) {
            boundary(dependencies, changed, profile)?;
        }
    }
    Ok(())
}

/// Retain every original publication so atomic refusal validates all twelve output views.
type Prepared = (
    fixture::Case<f64>,
    [nrz_matrix::PublishedPhasorSolve<f64>; 2],
    nrz_matrix::PhasorStatistics<f64>,
);

fn prepare(dependencies: DependencyPolicy) -> TestResult<Prepared> {
    let mut case = fixture::lifecycle_case(dependencies, &mut super::budget()?)?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let statistics = case.heap.evaluate_phasor_statistics(
        &executions,
        &ArithmeticContext::new()?,
        &mut ledger()?,
    )?;
    Ok((case, means, statistics))
}

fn boundary(dependencies: DependencyPolicy, changed: usize, profile: [u64; 3]) -> TestResult {
    let (mut case, means, statistics) = prepare(dependencies)?;
    let original = case.heap.reading_entries(&case.bindings[changed])?;
    let before = super::lifecycle::fingerprint(&case, &means)?;
    for (kind, needed) in profile
        .into_iter()
        .enumerate()
        .filter(|(_, needed)| *needed > 0)
    {
        let mut limits = profile;
        limits[kind] -= 1;
        refused(
            &mut case,
            changed,
            &original,
            &mut policy(limits, 0)?,
            needed,
        );
        assert_eq!(super::lifecycle::fingerprint(&case, &means)?, before);
        readable(&case.heap, &statistics)?;
    }
    refused(
        &mut case,
        changed,
        &original,
        &mut policy(profile, 1)?,
        profile[0] + 1,
    );
    assert_eq!(super::lifecycle::fingerprint(&case, &means)?, before);
    readable(&case.heap, &statistics)?;
    let mut resources = policy(profile, 0)?;
    case.heap
        .replace_reading_entries(&case.bindings[changed], &original, &mut resources)?;
    assert_eq!(resources.usage().work, profile[0]);
    assert!(
        statistics
            .moments(&case.heap, &ArithmeticContext::new()?)
            .is_err()
    );
    Ok(())
}

fn refused(
    case: &mut fixture::Case<f64>,
    changed: usize,
    original: &[f64],
    resources: &mut Budget,
    required: u64,
) {
    let result = case
        .heap
        .replace_reading_entries(&case.bindings[changed], original, resources);
    assert!(
        matches!(result, Err(LiveBindingError::Exact(ref fault)) if matches!(fault.kind, FaultKind::Exhausted { needed, .. } if needed == required))
    );
}

fn policy(profile: [u64; 3], prior: u64) -> TestResult<Budget> {
    let limits = Limits {
        work: profile[0],
        staged_payload_bytes: profile[1],
        output_bytes: profile[2],
        ..Limits::default()
    };
    let mut resources = Budget::new(limits)?;
    resources.work(prior, Location::new("prior driven write"))?;
    Ok(resources)
}

fn readable(heap: &nrz_matrix::Heap, statistics: &nrz_matrix::PhasorStatistics<f64>) -> TestResult {
    let context = ArithmeticContext::new()?;
    statistics.means(heap, &context)?;
    statistics.moments(heap, &context)?;
    statistics.parameter_errors(heap, &context)?;
    Ok(())
}
