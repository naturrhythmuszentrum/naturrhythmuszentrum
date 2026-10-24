// SPDX-License-Identifier: AGPL-3.0-only
//! Certified enclosed RHS parameters feed differential solves through the original factors.

use crate::kernel_oracle::Rational;
use crate::solve_testing::{TestResult, draft};
use crate::statistics::{Matrix, enclosed};
use crate::{Binding, ExecutionLimits, Heap, HeapLimits};
use nrz_scalar::ArithmeticContext;

/// The differential parameter enclosure is retained even when its represented candidate has zero residual.
/// ´test:crate:solve-differential-includes-rhs-parameter-error´
#[test]
fn solve_differential_includes_rhs_parameter_error() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, source] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 2)?.check(limits)?,
        vec![operator.value(), source.value()],
        limits,
    )
    .validate(&mut heap)?;
    let run = crate::solve::scratch::Run::new(
        &bound,
        1,
        &nrz_bindings::Budget::new(nrz_bindings::Limits::default())?,
    )?;
    let factor = bound.factor()?;
    let views = bound.normalized_inputs()?;
    let context = ArithmeticContext::new()?;
    let mut rhs = Matrix::zeros(1, 2)?;
    rhs.set(0, 0, 2.0, enclosed(2.0, 0.25, &context)?)?;
    rhs.set(0, 1, 4.0, enclosed(4.0, 0.0, &context)?)?;
    let (result, proof) = crate::solve::sensitivity::enclosed_solve(
        views.views().first().ok_or("operator")?,
        &rhs,
        &factor,
        &mut crate::statistics::contraction::Work::new(2),
        &context,
        crate::solve::scratch::Attempt {
            run: &run,
            index: 0,
        },
    )?;
    let center = *result.centers.first().ok_or("center")?;
    assert!(Rational::new(7, 8)?.covered(center, proof.radius())?);
    assert!(Rational::new(9, 8)?.covered(center, proof.radius())?);
    Ok(())
}

/// The propagated differential retains the original model allocation and separate parameter targets.
/// ´test:crate:solve-differential-keeps-original-model-and-parameter-targets´
#[test]
fn solve_differential_keeps_original_model_and_parameter_targets() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let original = std::rc::Rc::clone(&heap.joint.records.first().ok_or("joint record")?.model);
    let outcome = crate::kernel_testing::execute(
        &mut heap,
        draft(1, 2)?,
        vec![operator.value(), rhs.value()],
    )?;
    let result = crate::kernel_testing::output(&outcome)?;
    let reference = heap.moment_companion(result.value())?;
    let record = heap
        .statistics
        .records
        .get(reference.index)
        .ok_or("moment record")?;
    assert!(std::rc::Rc::ptr_eq(&original, &record.lineage.model.data));
    let jacobian = record.jacobian.as_ref().ok_or("Jacobian")?;
    assert_ne!(jacobian.regions[1], record.covariance.regions[1]);
    assert_ne!(jacobian.regions[1], result.value().key);
    let moments = heap.moments(reference)?;
    assert!(Rational::new(1, 16)?.covered(
        moments.covariance().get(0, 0)?,
        moments.covariance().error(0, 0)?
    )?);
    Ok(())
}

/// A complete source-dependent solve retains the exact derivative and all endpoint owners.
/// ´test:crate:solve-sensitivity-inventory-counts-both-endpoints´
#[test]
fn solve_sensitivity_inventory_counts_both_endpoints() -> TestResult {
    sensitivity_inventory::<f32>()?;
    sensitivity_inventory::<f64>()
}
/// Both formats have the same logical schedule and separate primitive-float counts.
fn sensitivity_inventory<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[P::ONE, P::from_u8(3), P::ONE], true)?;
    let outcome = crate::kernel_testing::execute(
        &mut heap,
        draft(1, 2)?,
        vec![operator.value(), rhs.value()],
    )?;
    let inventory = heap
        .operation_record(outcome.record())?
        .solve_inventory()
        .ok_or("solve inventory")?;
    assert!(inventory.derivative().is_some());
    assert_eq!(inventory.allocations().len(), 74);
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .map(crate::SolveAllocation::entries)
            .sum::<usize>(),
        279
    );
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .map(crate::SolveAllocation::float_entries)
            .sum::<usize>(),
        351
    );
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .filter_map(crate::SolveAllocation::owned_bytes)
            .sum::<usize>(),
        273 * size_of::<P>()
    );
    assert_eq!(inventory.peak_payload_bytes(), 102 * size_of::<P>());
    assert_eq!(
        (
            inventory.metadata_bytes(),
            inventory.planning_bytes(),
            inventory.source_bytes(),
            inventory.receipt_bytes()
        ),
        (8_520, 6_120, 1_144, 1_256)
    );
    for direction in 0..=3 {
        assert_eq!(
            inventory
                .allocations()
                .iter()
                .filter(|entry| entry.sensitivity_solve() == Some(direction))
                .count(),
            7
        );
    }
    Ok(())
}

/// Both endpoint type and complete original direction must match before an allocator can run.
/// ´test:crate:solve-sensitivity-inventory-refuses-wrong-roles-and-endpoints´
#[test]
fn solve_sensitivity_inventory_refuses_wrong_roles_and_endpoints() -> TestResult {
    use crate::solve::scratch::{Run, Stage};
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 2)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let run = Run::new(
        &bound,
        1,
        &nrz_bindings::Budget::new(nrz_bindings::Limits::default())?,
    )?;
    assert!(matches!(
        run.buffer::<f64>(Stage::SensitivityTruth, Some(0)),
        Err(crate::OperationError::PrivateAllocation(
            crate::PrivateAllocationError::Representation
        ))
    ));
    assert!(matches!(
        run.buffer::<nrz_scalar::Enclosure<f64>>(Stage::SensitivityCenters, Some(0)),
        Err(crate::OperationError::PrivateAllocation(
            crate::PrivateAllocationError::Representation
        ))
    ));
    assert!(matches!(
        run.buffer::<f64>(Stage::SensitivityCenters, Some(4)),
        Err(crate::OperationError::PrivateAllocation(
            crate::PrivateAllocationError::UnplannedMeaning
        ))
    ));
    assert!(matches!(
        run.buffer::<f64>(Stage::StatisticalMeans, None),
        Err(crate::OperationError::PrivateAllocation(
            crate::PrivateAllocationError::UnplannedMeaning
        ))
    ));
    Ok(())
}
/// Equal parameter shapes cannot exchange the actual original candidate owners.
/// ´test:crate:solve-statistics-publication-refuses-exchanged-parameter-permits´
#[test]
fn solve_statistics_publication_refuses_exchanged_parameter_permits() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let count = heap.descriptors.iter().count();
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 2)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let moment = candidate.statistics.first_mut().ok_or("moments")?;
    std::mem::swap(
        &mut moment.jacobian.centers,
        &mut moment.sensitivity.centers,
    );
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("exchanged owners published")?;
    assert!(matches!(
        failure.cause(),
        crate::OperationError::PrivateAllocation(crate::PrivateAllocationError::UnticketedBuffer)
    ));
    assert_eq!(heap.descriptors.iter().count(), count);
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
/// The optional affine mean path has distinct input, backend, truth and publication owners.
/// ´test:crate:solve-mean-inventory-retains-the-explicit-affine-path´
#[test]
fn solve_mean_inventory_retains_the_explicit_affine_path() -> TestResult {
    mean_inventory::<f32>()?;
    mean_inventory::<f64>()
}
/// A fixed operator preserves affine moments without promoting an operating point into a mean.
fn mean_inventory<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] = crate::solve_statistics_testing::common(
        &mut heap,
        &[P::ZERO, P::ONE, P::from_u8(2)],
        true,
    )?;
    let outcome = crate::kernel_testing::execute(
        &mut heap,
        draft(1, 2)?,
        vec![operator.value(), rhs.value()],
    )?;
    let inventory = heap
        .operation_record(outcome.record())?
        .solve_inventory()
        .ok_or("inventory")?;
    assert_eq!(inventory.allocations().len(), 83);
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .map(crate::SolveAllocation::entries)
            .sum::<usize>(),
        304
    );
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .map(crate::SolveAllocation::float_entries)
            .sum::<usize>(),
        381
    );
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .filter_map(crate::SolveAllocation::owned_bytes)
            .sum::<usize>(),
        292 * size_of::<P>()
    );
    assert_eq!(inventory.peak_payload_bytes(), 102 * size_of::<P>());
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .filter(|entry| entry.contraction() == Some(3))
            .count(),
        5
    );
    assert_eq!(
        inventory
            .allocations()
            .iter()
            .filter(|entry| entry.parameter() == Some(3))
            .count(),
        2
    );
    Ok(())
}

/// Statistical source selections and the entire finite schedule obey both explicit metadata ceilings.
/// ´test:crate:solve-statistical-inventory-metadata-boundaries´
#[test]
fn solve_statistical_inventory_metadata_boundaries() -> TestResult {
    let required = statistical_budget(None)?.ok_or("default receipt")?;
    assert_eq!(required, 8_520);
    for staged in [false, true] {
        assert_eq!(
            statistical_budget(Some((staged, required)))?,
            Some(required)
        );
        assert_eq!(statistical_budget(Some((staged, required - 1)))?, None);
    }
    Ok(())
}
/// Caller metadata ceilings remain independent of existing factor and numerical work admission.
fn statistical_budget(ceiling: Option<(bool, usize)>) -> TestResult<Option<usize>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 2)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let factor = bound.factor()?;
    let mut policy = nrz_bindings::Limits::default();
    if let Some((staged, bytes)) = ceiling {
        if staged {
            policy.staged_payload_bytes = u64::try_from(bytes)?;
        } else {
            policy.output_bytes = u64::try_from(bytes)?;
        }
    }
    let budget = nrz_bindings::Budget::new(policy)?;
    let result = bound.evaluate_factored_with_inventory_budget(
        &bound.normalized_inputs()?,
        &factor,
        &budget,
    );
    match result {
        Ok(candidate) => {
            let outcome = bound.publish(candidate)?;
            Ok(Some(
                heap.operation_record(outcome.record())?
                    .solve_inventory()
                    .ok_or("receipt")?
                    .metadata_bytes(),
            ))
        }
        Err(failure) => {
            assert!(matches!(failure.cause(), crate::OperationError::Exact(_)));
            drop(bound);
            assert_eq!(heap.provenance_usage().records(), 0);
            Ok(None)
        }
    }
}
