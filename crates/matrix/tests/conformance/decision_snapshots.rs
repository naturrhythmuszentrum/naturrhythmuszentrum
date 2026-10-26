// SPDX-License-Identifier: AGPL-3.0-only
//! Existing LC input capture and publication are measured before live decision admission.

use crate::binding_testing::{TestResult, shape};
use nrz_bindings::{Budget, Descriptor, Dimension, Kind, Limits, Measure, Role};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, InputSnapshot, OperationDraft,
    OperationError, ProvenanceLimits, PublishedBundle,
};

/// The two physical inputs of the illustrative duration/period decision enclosure.
fn inputs(heap: &mut Heap) -> TestResult<[PublishedBundle<f64>; 2]> {
    let time = time_descriptor()?;
    let extent = Extent::new(1, 1)?;
    Ok([
        heap.import_declared_bounds(extent, &time, &[1311.0_f64 / 131_072.0], &[1.0 / 262_144.0])?,
        heap.import_declared_bounds(extent, &time, &[103.0_f64 / 32_768.0], &[1.0 / 262_144.0])?,
    ])
}

/// Both original coordinates are coherent time differences.
fn time_descriptor() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(
            "time".into(),
            Box::new(Dimension::integers([0, 0, 1, 0, 0, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Measure the existing capture and current-record check without claiming a decision API.
/// ´test:integration:decision-snapshot-baseline-record´
#[test]
fn decision_snapshot_baseline_record() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = inputs(&mut heap)?;
    let stamps = [
        heap.value(source[0].value())?.stamp(),
        heap.value(source[1].value())?.stamp(),
    ];
    let recipe = OperationDraft::copy(vec![shape(1, 1, &time_descriptor()?)?; 2])
        .check(ExecutionLimits::default())?;
    let budget = Budget::new(Limits::default())?;
    let bound = Binding::new(
        recipe,
        source.map(PublishedBundle::value).to_vec(),
        ExecutionLimits::default(),
    )
    .validate_with_resource_budget(&mut heap, &budget)?;
    let captured = bound.dependencies().to_vec();
    assert_eq!(captured.len(), 2);
    for (input, stamp) in captured.iter().zip(stamps) {
        assert_eq!(input.stamp(), stamp);
        assert_eq!(input.companion().ok_or("missing companion")?.stamp(), stamp);
    }
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let published = bound.publish(candidate)?;
    let record = heap.operation_record(published.record())?;
    assert_eq!(record.inputs().len(), 2);
    let usage = heap.provenance_usage();
    assert_eq!(size_of::<InputSnapshot<f64>>(), 592);
    assert_eq!(budget.usage().work, 0);
    assert_eq!(
        (
            usage.depth(),
            usage.records(),
            usage.edges(),
            usage.bytes(),
            usage.work()
        ),
        (1, 3, 6, 2458, 6)
    );
    heap.replace_declared_bounds(
        source[0].value(),
        stamps[0].version(),
        &[1311.0 / 131_072.0],
        &[1.0 / 1024.0],
    )?;
    assert!(matches!(
        heap.operation_record(published.record()),
        Err(HeapError::StaleEvidence)
    ));
    assert_eq!(
        heap.historical_record(published.record())?.inputs().len(),
        2
    );
    Ok(())
}

/// The two one-entry values and their bounds require four entries per capture scan.
/// ´test:integration:decision-snapshot-baseline-entry-limits´
#[test]
fn decision_snapshot_baseline_entry_limits() -> TestResult {
    for limits in [
        ExecutionLimits::new(2, 4, 4),
        ExecutionLimits::new(2, 3, 4),
        ExecutionLimits::new(2, 4, 3),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = inputs(&mut heap)?;
        let stamps = [
            heap.value(source[0].value())?.stamp(),
            heap.value(source[1].value())?.stamp(),
        ];
        let before = heap.provenance_usage();
        let recipe = OperationDraft::copy(vec![shape(1, 1, &time_descriptor()?)?; 2])
            .check(ExecutionLimits::default())?;
        let result = Binding::new(recipe, source.map(PublishedBundle::value).to_vec(), limits)
            .validate(&mut heap);
        if limits == ExecutionLimits::new(2, 4, 4) {
            let bound = result?;
            assert_eq!(bound.dependencies().len(), 2);
            drop(bound);
        } else {
            assert!(matches!(result, Err(OperationError::ResourceLimit)));
        }
        assert_eq!(heap.provenance_usage(), before);
        for (bundle, stamp) in source.into_iter().zip(stamps) {
            assert_eq!(heap.value(bundle.value())?.stamp(), stamp);
            let radius = heap.companion(bundle.bound().ok_or("missing bound")?)?;
            assert_eq!(radius.target_stamp(), stamp);
            assert_eq!(
                radius.region().get(0, 0)?.to_bits(),
                (1.0_f64 / 262_144.0).to_bits()
            );
        }
    }
    Ok(())
}

/// Publication metadata and traversal limits have their own measured one-below refusals.
/// ´test:integration:decision-snapshot-baseline-publication-limits´
#[test]
fn decision_snapshot_baseline_publication_limits() -> TestResult {
    for (bytes, work, accepted) in [(2458, 6, true), (2457, 6, false), (2458, 5, false)] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = inputs(&mut heap)?;
        let before = heap.provenance_usage();
        let stamp = heap.value(source[0].value())?.stamp();
        heap.set_provenance_limits(ProvenanceLimits::new(1, 3, 6, bytes, work))?;
        let recipe = OperationDraft::copy(vec![shape(1, 1, &time_descriptor()?)?; 2])
            .check(ExecutionLimits::default())?;
        let bound = Binding::new(
            recipe,
            source.map(PublishedBundle::value).to_vec(),
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate);
        if accepted {
            let published = result?;
            assert_eq!(
                heap.operation_record(published.record())?.outputs().len(),
                2
            );
            assert_eq!(heap.provenance_usage().bytes(), 2458);
            assert_eq!(heap.provenance_usage().work(), 6);
        } else {
            let failure = result.err().ok_or("short publication budget accepted")?;
            assert!(matches!(failure.cause(), OperationError::ProvenanceLimit));
            assert_eq!(heap.provenance_usage(), before);
        }
        assert_eq!(heap.value(source[0].value())?.stamp(), stamp);
        assert_eq!(
            heap.companion(source[0].bound().ok_or("missing bound")?)?
                .target_stamp(),
            stamp
        );
    }
    Ok(())
}
