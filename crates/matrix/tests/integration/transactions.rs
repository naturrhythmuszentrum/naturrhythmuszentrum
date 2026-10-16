// SPDX-License-Identifier: AGPL-3.0-only
//! Complete publication, lineage, compaction and invalidation lifecycles.

use crate::binding_testing::{TestResult, operation, shape};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationDraft,
    OperationError, ProvenanceLimits,
};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Exercise complete exact-copy publication for each sealed primitive format.
fn publish_format<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let original = [
        P::min_positive_normal().next_down(),
        P::max_finite(),
        -P::ONE,
        P::ZERO,
    ];
    let source = heap.import_declared_bounds(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &original,
        &[P::ZERO; 4],
    )?;
    let original_stamp = heap.value(source.value())?.stamp();
    let bound = Binding::new(
        operation(2, 2)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let reserved = bound
        .output_reservations()
        .first()
        .ok_or("missing reservation")?
        .stamp();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = *outcome.output().first().ok_or("missing output")?;
    let value = heap.value(output.value())?;
    assert_eq!(value.stamp(), reserved);
    assert_ne!(value.stamp().id(), original_stamp.id());
    assert_eq!(value.coverage(), Coverage::DeclaredBounds);
    assert!(
        value
            .region()
            .export()?
            .iter()
            .zip(original)
            .all(|(actual, expected)| actual.same_bits(expected))
    );
    let companion = heap.companion(output.bound().ok_or("missing output bound")?)?;
    assert_eq!(companion.target(), output.value());
    assert_eq!(companion.target_stamp(), reserved);
    let record = heap.operation_record(outcome.record())?;
    assert_eq!(
        record
            .inputs()
            .first()
            .ok_or("missing input record")?
            .stamp(),
        original_stamp
    );
    let node = record
        .outputs()
        .first()
        .ok_or("missing output record")?
        .provenance();
    assert_eq!(heap.provenance(node)?.depth(), 1);
    assert_eq!(heap.provenance(node)?.parents(), []);
    heap.compact()?;
    assert_eq!(heap.value(output.value())?.stamp(), reserved);
    assert_eq!(heap.operation_record(outcome.record())?.evidence().len(), 1);
    Ok(())
}

/// Complete publication preserves extrema and companion targets for both formats.
/// ´test:integration:copy-publication-preserves-bits-and-retargets-companions´
#[test]
fn copy_publication_preserves_bits_and_retargets_companions() -> TestResult {
    publish_format::<f32>()?;
    publish_format::<f64>()
}

/// Copy chains keep explicit parent edges and become stale after source retirement and reuse.
/// ´test:integration:copy-lineage-survives-compaction-and-refuses-retired-source´
#[test]
fn copy_lineage_survives_compaction_and_refuses_retired_source() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let recipe = operation(1, 1)?;
    let mut current = source;
    let mut parent = None;
    let mut records = Vec::new();
    for depth in 1..=3 {
        let bound = Binding::new(
            recipe.clone(),
            vec![current.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outcome = bound.publish(candidate)?;
        current = *outcome.output().first().ok_or("missing output")?;
        let id = heap
            .operation_record(outcome.record())?
            .outputs()
            .first()
            .ok_or("missing output record")?
            .provenance();
        assert_eq!(heap.provenance(id)?.parents(), parent.as_slice());
        assert_eq!(heap.provenance(id)?.depth(), depth);
        records.push(outcome.record());
        parent = Some(id);
    }
    heap.compact()?;
    assert_eq!(
        heap.value(current.value())?.region().get(0, 0)?.to_bits(),
        2.0_f64.to_bits()
    );
    heap.retire(source.value())?;
    heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    assert!(matches!(
        heap.value(current.value()),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(
        heap.companion(current.bound().ok_or("missing bound")?),
        Err(HeapError::StaleEvidence)
    ));
    for record in records {
        assert!(matches!(
            heap.operation_record(record),
            Err(HeapError::StaleEvidence)
        ));
        assert_eq!(heap.historical_record(record)?.evidence().len(), 1);
    }
    Ok(())
}

/// Equal dimensions and fresh coordinate names cannot reinterpret a published ordered shape.
/// ´test:integration:published-shape-retains-ordered-axis-identities´
#[test]
fn published_shape_retains_ordered_axis_identities() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let recipe = operation(1, 1)?;
    let bound = Binding::new(
        recipe.clone(),
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let published = bound.publish(candidate)?;
    let output = published.output().first().ok_or("missing output")?.value();
    assert!(matches!(
        Binding::new(operation(1, 1)?, vec![output], ExecutionLimits::default())
            .validate(&mut heap),
        Err(OperationError::InputMismatch { slot: 0 })
    ));
    assert_eq!(
        Binding::new(recipe, vec![output], ExecutionLimits::default())
            .validate(&mut heap)?
            .output_reservations()
            .len(),
        1
    );
    Ok(())
}

/// Empty connected outputs retain complete declared companion structure and fresh versions.
/// ´test:integration:empty-copy-bundle-publishes-completely´
#[test]
fn empty_copy_bundle_publishes_completely() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(6, 0, 2))?;
    let source =
        heap.import_declared_bounds(Extent::new(0, 2)?, &Descriptor::ratio(), &[] as &[f64], &[])?;
    let recipe = OperationDraft::copy(vec![shape(0, 2, &Descriptor::ratio())?; 2])
        .check(ExecutionLimits::default())?;
    let bound = Binding::new(recipe, vec![source.value(); 2], ExecutionLimits::default())
        .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let published = bound.publish(candidate)?;
    for output in published.output() {
        assert_eq!(heap.value(output.value())?.region().export()?, []);
        assert_eq!(
            heap.companion(output.bound().ok_or("missing bound")?)?
                .region()
                .export()?,
            []
        );
    }
    assert_eq!(
        heap.operation_record(published.record())?.outputs().len(),
        2
    );
    Ok(())
}

/// Failed depth growth leaves the first output current and the full attempted copy inspectable.
/// ´test:integration:lineage-depth-limit-does-not-publish-a-partial-child´
#[test]
fn lineage_depth_limit_does_not_publish_a_partial_child() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    heap.set_provenance_limits(ProvenanceLimits::new(1, 100, 100, 100_000, 1000))?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let recipe = operation(1, 1)?;
    let first = Binding::new(
        recipe.clone(),
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = first.evaluate(first.normalized_inputs()?)?;
    let output = first.publish(candidate)?;
    let current = output.output().first().ok_or("missing output")?.value();
    let before = heap.provenance_usage();
    let next =
        Binding::new(recipe, vec![current], ExecutionLimits::default()).validate(&mut heap)?;
    let candidate = next.evaluate(next.normalized_inputs()?)?;
    let refusal = next.publish(candidate).err().ok_or("depth exceeded")?;
    assert!(matches!(refusal.cause(), OperationError::ProvenanceLimit));
    assert_eq!(refusal.record().evidence(&heap, 0)?.entries(), 1);
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(
        heap.value(current)?.region().get(0, 0)?.to_bits(),
        2.0_f64.to_bits()
    );
    Ok(())
}

/// Reclaiming stale storage preserves historical records and never lends stale evidence.
/// ´test:integration:stale-derived-bundles-can-retire-and-release-capacity´
#[test]
fn stale_derived_bundles_can_retire_and_release_capacity() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(5, 4, 2))?;
    let extent = Extent::new(1, 1)?;
    let source = heap.import_declared_bounds(extent, &Descriptor::ratio(), &[2.0_f64], &[0.25])?;
    let version = heap.value(source.value())?.stamp().version();
    let bound = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = *outcome.output().first().ok_or("missing output")?;
    let alias = heap.alias(output.value(), &nrz_matrix::Layout::contiguous(extent)?)?;
    heap.replace_declared_bounds(source.value(), version, &[2.0], &[0.125])?;
    assert!(matches!(
        heap.value(output.value()),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(
        heap.companion(output.bound().ok_or("missing bound")?),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(heap.compact(), Err(HeapError::StaleEvidence)));
    heap.retire(output.value())?;
    assert!(matches!(
        heap.value(output.value()),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(heap.value(alias), Err(HeapError::StaleHandle)));
    let fresh = heap.import_declared_bounds(extent, &Descriptor::ratio(), &[3.0_f64], &[0.5])?;
    heap.compact()?;
    assert_eq!(
        heap.value(fresh.value())?.region().get(0, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(heap.historical_record(outcome.record())?.outputs().len(), 1);
    assert!(matches!(
        heap.operation_record(outcome.record()),
        Err(HeapError::StaleEvidence)
    ));
    Ok(())
}
