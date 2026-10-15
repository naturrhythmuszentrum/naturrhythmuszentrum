// SPDX-License-Identifier: AGPL-3.0-only
//! Public execution identity, provenance limit and staleness refusal twins.

use crate::binding_testing::{TestResult, operation, shape};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, Layout, OperationDraft,
    OperationError, ProvenanceLimits, ProvenanceUsage, PublishedBundle,
};
use nrz_units::Descriptor;

/// Publish a copy through the complete public operation boundary.
fn copy(
    heap: &mut Heap,
    input: PublishedBundle<f64>,
) -> TestResult<nrz_matrix::Outcome<Vec<PublishedBundle<f64>>>> {
    let bound = Binding::new(
        operation(1, 1)?,
        vec![input.value()],
        ExecutionLimits::default(),
    )
    .validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    Ok(bound.publish(candidate)?)
}

/// Foreign normalized bundles and candidates refuse even when recipe and bytes agree.
/// ´test:integration:execution-gates-refuse-foreign-inputs-and-candidates´
#[test]
fn execution_gates_refuse_foreign_inputs_and_candidates() -> TestResult {
    let mut first_heap = Heap::new(HeapLimits::default())?;
    let mut second_heap = Heap::new(HeapLimits::default())?;
    let first =
        first_heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let second =
        second_heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let left = Binding::new(
        operation(1, 1)?,
        vec![first.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut first_heap)?;
    let right = Binding::new(
        operation(1, 1)?,
        vec![second.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut second_heap)?;
    let failure = right
        .evaluate(left.normalized_inputs()?)
        .err()
        .ok_or("foreign bundle admitted")?;
    assert!(matches!(failure.cause(), OperationError::ForeignInputs));
    let candidate = left.evaluate(left.normalized_inputs()?)?;
    let repeated = left
        .evaluate(left.normalized_inputs()?)
        .err()
        .ok_or("second candidate admitted")?;
    assert!(matches!(repeated.cause(), OperationError::AlreadyEvaluated));
    let failure = right
        .publish(candidate)
        .err()
        .ok_or("foreign candidate admitted")?;
    assert!(matches!(failure.cause(), OperationError::ForeignCandidate));
    assert_eq!(failure.record().historical_evidence(), []);
    assert_eq!(second_heap.provenance_usage().records(), 0);
    drop(left);
    assert_eq!(copy(&mut second_heap, second)?.output().len(), 1);
    Ok(())
}

/// Construct one limit variation from the exact accounted requirements of a successful copy.
fn limit_variant(usage: ProvenanceUsage, resource: usize, value: usize) -> ProvenanceLimits {
    let mut values = [
        usage.depth(),
        usage.records(),
        usage.edges(),
        usage.bytes(),
        usage.work(),
    ];
    if let Some(selected) = values.get_mut(resource) {
        *selected = value;
    }
    let [depth, records, edges, bytes, work] = values;
    ProvenanceLimits::new(depth, records, edges, bytes, work)
}

/// All five graph limits are tested immediately below, at and above the required account.
/// ´test:integration:provenance-limit-boundaries-preserve-partial-evidence´
#[test]
fn provenance_limit_boundaries_preserve_partial_evidence() -> TestResult {
    let mut baseline = Heap::new(HeapLimits::default())?;
    let value = baseline.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    copy(&mut baseline, value)?;
    let usage = baseline.provenance_usage();
    for (resource, required) in [
        usage.depth(),
        usage.records(),
        usage.edges(),
        usage.bytes(),
        usage.work(),
    ]
    .into_iter()
    .enumerate()
    {
        for limit in [required - 1, required, required + 1] {
            check_limit(usage, resource, required, limit)?;
        }
    }
    Ok(())
}

/// A capacity refusal leaves one independently valid copy claim and no graph publication.
fn check_limit(
    usage: ProvenanceUsage,
    resource: usize,
    required: usize,
    limit: usize,
) -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    heap.set_provenance_limits(limit_variant(usage, resource, limit))?;
    let input = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let stamp = heap.value(input.value())?.stamp();
    let bound = Binding::new(
        operation(1, 1)?,
        vec![input.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let result = bound.publish(candidate);
    if limit < required {
        let failure = result.err().ok_or("resource limit admitted")?;
        assert!(matches!(failure.cause(), OperationError::ProvenanceLimit));
        assert_eq!(failure.record().evidence(&heap, 0)?.entries(), 1);
        assert_eq!(heap.provenance_usage().records(), 0);
    } else {
        assert_eq!(result?.output().len(), 1);
        assert_eq!(heap.provenance_usage(), usage);
    }
    assert_eq!(heap.value(input.value())?.stamp(), stamp);
    Ok(())
}

/// Companion-only mutation invalidates just its dependent output through existing accessors.
/// ´test:integration:companion-change-stales-only-dependent-output´
#[test]
fn companion_change_stales_only_dependent_output() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(1, 1)?;
    let first = heap.import_declared_bounds(extent, &Descriptor::ratio(), &[2.0_f64], &[0.25])?;
    let second = heap.import_declared_bounds(extent, &Descriptor::ratio(), &[3.0_f64], &[0.5])?;
    let version = heap.value(first.value())?.stamp().version();
    let recipe = OperationDraft::copy(vec![shape(1, 1, &Descriptor::ratio())?; 2])
        .check(ExecutionLimits::default())?;
    let bound = Binding::new(
        recipe,
        vec![first.value(), second.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let published = bound.publish(candidate)?;
    let left = *published.output().first().ok_or("missing first output")?;
    let right = *published.output().get(1).ok_or("missing second output")?;
    let left_bound = left.bound().ok_or("missing first bound")?;
    let right_bound = right.bound().ok_or("missing second bound")?;
    assert_eq!(
        heap.companion(left_bound)?.region().get(0, 0)?.to_bits(),
        0.25_f64.to_bits()
    );
    heap.replace_declared_bounds(first.value(), version, &[2.0], &[0.125])?;
    assert!(matches!(
        heap.companion(left_bound),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(
        heap.value(left.value()),
        Err(HeapError::StaleEvidence)
    ));
    assert_eq!(
        heap.companion(right_bound)?.region().get(0, 0)?.to_bits(),
        0.5_f64.to_bits()
    );
    assert!(matches!(
        heap.operation_record(published.record()),
        Err(HeapError::StaleEvidence)
    ));
    assert_eq!(
        heap.historical_record(published.record())?.outputs().len(),
        2
    );
    Ok(())
}

/// Even disjoint aliases share the whole-base version; an unrelated sibling stays current.
/// ´test:integration:alias-write-stales-dependent-companion-access´
#[test]
fn alias_write_stales_dependent_companion_access() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64, 3.0],
        &[0.25, 0.5],
    )?;
    let first = heap.alias(
        source.value(),
        &Layout::gather(Extent::new(1, 1)?, 2, &[0], 1)?,
    )?;
    let last = heap.alias(
        source.value(),
        &Layout::gather(Extent::new(1, 1)?, 2, &[1], 1)?,
    )?;
    let other = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[4.0_f64],
        &[0.125],
    )?;
    let version = heap.value(source.value())?.stamp().version();
    let recipe = OperationDraft::copy(vec![shape(1, 1, &Descriptor::ratio())?; 2])
        .check(ExecutionLimits::default())?;
    let bound = Binding::new(
        recipe,
        vec![last, other.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let published = bound.publish(candidate)?;
    let selected = published
        .output()
        .first()
        .ok_or("missing selected output")?
        .bound()
        .ok_or("missing selected bound")?;
    let sibling = published
        .output()
        .get(1)
        .ok_or("missing sibling")?
        .bound()
        .ok_or("missing sibling bound")?;
    heap.replace_declared_bounds(first, version, &[7.0], &[0.25])?;
    assert!(matches!(
        heap.companion(selected),
        Err(HeapError::StaleEvidence)
    ));
    assert_eq!(
        heap.companion(sibling)?.region().get(0, 0)?.to_bits(),
        0.125_f64.to_bits()
    );
    assert_eq!(
        heap.value(last)?.region().get(0, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    Ok(())
}

/// Record and provenance locators are scoped before any foreign graph index is consulted.
/// ´test:integration:operation-evidence-identities-refuse-foreign-heaps´
#[test]
fn operation_evidence_identities_refuse_foreign_heaps() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let outcome = copy(&mut heap, source)?;
    let record = heap.operation_record(outcome.record())?;
    let node = record
        .outputs()
        .first()
        .ok_or("missing output record")?
        .provenance();
    let foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        foreign.historical_record(outcome.record()),
        Err(HeapError::ForeignHandle)
    ));
    assert!(matches!(
        foreign.operation_record(outcome.record()),
        Err(HeapError::ForeignHandle)
    ));
    assert!(matches!(
        foreign.provenance(node),
        Err(HeapError::ForeignHandle)
    ));
    assert_eq!(
        heap.provenance(node)?.source().stamp(),
        heap.value(source.value())?.stamp()
    );
    assert_eq!(heap.provenance_usage().records(), 2);
    assert_eq!(heap.provenance_usage().edges(), 3);
    Ok(())
}

/// Gather dependencies charge both exact map comparison and map validation on access.
/// ´test:integration:provenance-work-counts-repeated-map-inspection´
#[test]
fn provenance_work_counts_repeated_map_inspection() -> TestResult {
    for work in [8, 9, 10] {
        let mut heap = Heap::new(HeapLimits::default())?;
        heap.set_provenance_limits(ProvenanceLimits::new(2, 10, 10, 100_000, work))?;
        let source =
            heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 3.0])?;
        let selected = heap.alias(
            source.value(),
            &Layout::gather(Extent::new(3, 1)?, 2, &[0, 1, 0], 3)?,
        )?;
        let bound = Binding::new(operation(3, 1)?, vec![selected], ExecutionLimits::default())
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate);
        if work < 9 {
            assert!(matches!(
                result.err().ok_or("work limit admitted")?.cause(),
                OperationError::ProvenanceLimit
            ));
            assert_eq!(heap.provenance_usage().records(), 0);
        } else {
            let output = result?;
            assert_eq!(heap.provenance_usage().work(), 9);
            assert_eq!(heap.operation_record(output.record())?.outputs().len(), 1);
        }
    }
    Ok(())
}
