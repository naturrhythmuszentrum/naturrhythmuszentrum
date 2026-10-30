// SPDX-License-Identifier: AGPL-3.0-only
//! Precision conversions preserve original arenas and publish complete target maps.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, PrecisionContract,
};
use nrz_scalar::Precision;

/// Narrowing encloses the exact represented source and widening preserves its entire carried bound.
/// ´test:integration:precision-conversion-retains-errors-and-fresh-identities´
/// ´claim:matrix:precision-original-errors-and-identities´ Narrowing encloses the original nominal target and widening preserves its numerical error, total and fresh identities.
#[test]
fn precision_conversion_retains_errors_and_fresh_identities() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let contract = PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &quantity, &[0.1_f64])?;
    let original = heap.value(source.value())?.stamp();
    let limits = ExecutionLimits::default();
    let operation =
        OperationDraft::precision(vec![contract.clone()], Precision::F64, Precision::F32)
            .check(limits)?;
    let bound = Binding::<f64, f32>::convert_precision(operation, vec![source.value()], limits)
        .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let narrowed = *outcome.output().first().ok_or("narrowed")?;
    let center = f64::from(heap.value(narrowed.value())?.region().get(0, 0)?);
    let error = f64::from(
        heap.companion(narrowed.evaluation().ok_or("error")?)?
            .region()
            .get(0, 0)?,
    );
    assert!(covers(
        center,
        error,
        3_602_879_701_896_397,
        36_028_797_018_963_968
    )?);
    let mapping = heap.operation_record(outcome.record())?.target_map(0)?;
    assert_eq!(mapping.source().stamp(), original);
    assert_ne!(mapping.target().stamp().id(), original.id());
    assert_eq!(mapping.target().stamp().version().number(), 1);
    assert_eq!(mapping.source().precision(), Precision::F64);
    assert_eq!(mapping.target().precision(), Precision::F32);
    let prior = heap
        .companion(narrowed.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    let operation =
        OperationDraft::precision(vec![contract], Precision::F32, Precision::F64).check(limits)?;
    let bound = Binding::<f32, f64>::convert_precision(operation, vec![narrowed.value()], limits)
        .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let restored = bound.publish(candidate)?.into_output();
    let restored = restored.first().ok_or("restored")?;
    let radius = heap
        .companion(restored.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    assert_eq!(radius.to_bits(), f64::from(prior).to_bits());
    assert!(covers(
        heap.value(restored.value())?.region().get(0, 0)?,
        radius,
        3_602_879_701_896_397,
        36_028_797_018_963_968
    )?);
    assert_eq!(
        heap.companion(restored.evaluation().ok_or("error")?)?
            .region()
            .get(0, 0)?
            .to_bits(),
        error.to_bits()
    );
    assert_eq!(heap.value(source.value())?.stamp(), original);
    Ok(())
}

/// Every failing position leaves enough capacity to prove that no successful prefix was published.
/// ´test:integration:precision-bundle-failure-at-each-step-publishes-nothing´
#[test]
fn precision_bundle_failure_at_each_step_publishes_nothing() -> TestResult {
    for failure_slot in 0..3 {
        let declarations = owner()?;
        let quantity = Descriptor::ratio();
        let contract = PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
        let mut heap = Heap::new(HeapLimits::new(13, 10, 8))?;
        let mut centers = [1.0_f64; 3];
        *centers.get_mut(failure_slot).ok_or("failure position")? = f64::MAX;
        let source = heap.import_represented(Extent::new(3, 1)?, &quantity, &centers)?;
        let mut inputs = Vec::new();
        for slot in 0..3 {
            let selection =
                nrz_matrix::Layout::gather(Extent::new(1, 1)?, 3, std::slice::from_ref(&slot), 3)?;
            inputs.push(heap.alias(source.value(), &selection)?);
        }
        let usage = heap.provenance_usage();
        let limits = ExecutionLimits::default();
        let operation =
            OperationDraft::precision(vec![contract; 3], Precision::F64, Precision::F32)
                .check(limits)?;
        let bound = Binding::<f64, f32>::convert_precision(operation, inputs.clone(), limits)
            .validate(&mut heap)?;
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("overflow accepted")?;
        assert!(matches!(
            failure.cause(),
            nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Arithmetic(
                nrz_scalar::ArithmeticFailure::Overflow
            ))
        ));
        assert_eq!(failure.record().historical_evidence().len(), failure_slot);
        drop(bound);
        assert_eq!(heap.provenance_usage(), usage);
        for input in inputs {
            assert_eq!(heap.value(input)?.stamp().version().number(), 1);
        }
        let available = heap.import_represented(Extent::new(10, 1)?, &quantity, &[0.0_f32; 10])?;
        assert_eq!(
            heap.value(available.value())?.region().extent().entries(),
            10
        );
    }
    Ok(())
}

/// Existing declared bounds are outward-converted independently of the new center displacement.
/// ´test:integration:precision-conversion-outwardly-carries-prior-bounds´
#[test]
fn precision_conversion_outwardly_carries_prior_bounds() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let contract = PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_declared_bounds(Extent::new(1, 1)?, &quantity, &[1.0_f64], &[0.125])?;
    let limits = ExecutionLimits::default();
    let operation =
        OperationDraft::precision(vec![contract], Precision::F64, Precision::F32).check(limits)?;
    let bound = Binding::<f64, f32>::convert_precision(operation, vec![source.value()], limits)
        .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outputs = bound.publish(candidate)?.into_output();
    let output = outputs.first().ok_or("output")?;
    let center = f64::from(heap.value(output.value())?.region().get(0, 0)?);
    let radius = f64::from(
        heap.companion(output.bound().ok_or("total")?)?
            .region()
            .get(0, 0)?,
    );
    assert!(covers(center, radius, 7, 8)?);
    assert!(covers(center, radius, 9, 8)?);
    assert_eq!(
        heap.value(output.value())?.coverage(),
        nrz_matrix::Coverage::DeclaredBounds
    );
    assert_eq!(
        heap.companion(output.bound().ok_or("total")?)?.target(),
        output.value()
    );
    Ok(())
}
