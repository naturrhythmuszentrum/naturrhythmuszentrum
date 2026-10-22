// SPDX-License-Identifier: AGPL-3.0-only
//! Precision changes retain original models rather than importing rounded parameters as new sources.

use crate::conversion_testing::{TestResult, owner, shape};
use crate::statistics_testing::{check, vector, worked_covariance};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, CheckedShape, ExecutionLimits, Handle, Heap, HeapLimits, JointLimits, OperationDraft,
    PrecisionContract, PublishedBundle, Value,
};
use nrz_scalar::{Precision, Scalar};

/// Narrowing and widening preserve the original mathematical covariance and mixed-format target maps.
/// ´test:integration:statistical-precision-round-trip-retains-original-model-and-sources´
#[test]
fn statistical_precision_round_trip_retains_original_model_and_sources() -> TestResult {
    let declarations = owner()?;
    let coordinates = shape(&declarations, &Descriptor::ratio(), 2)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<f64>(&mut heap, true)?;
    let narrow = convert::<f64, f32>(&mut heap, coordinates.clone(), source.value())?;
    let view = heap.moments(heap.moment_companion(narrow.value())?)?;
    let model = view.model();
    let sources = view.source_ids().to_vec();
    check(view.covariance(), &worked_covariance()?)?;
    let wide = convert::<f32, f64>(&mut heap, coordinates.clone(), narrow.value())?;
    let view = heap.moments(heap.moment_companion(wide.value())?)?;
    assert_eq!(view.model(), model);
    assert_eq!(view.source_ids(), sources);
    check(view.covariance(), &worked_covariance()?)?;
    let cross =
        heap.materialize_cross_covariance(narrow.value(), source.value(), JointLimits::default())?;
    let view = heap.moments(cross)?;
    assert_eq!(view.target_maps()[0].precision(), Precision::F32);
    assert_eq!(view.target_maps()[1].precision(), Precision::F64);
    check(view.covariance(), &worked_covariance()?)?;
    let output = crate::kernel_testing::output(&crate::kernel_testing::execute(
        &mut heap,
        OperationDraft::add(
            coordinates.clone(),
            coordinates,
            nrz_matrix::ShapeLimits::default(),
        )?,
        vec![narrow.value(); 2],
    )?)?;
    let expected = worked_covariance()?
        .into_iter()
        .map(|value| value.multiply(crate::kernel_oracle::Rational::new(4, 1)?))
        .collect::<Result<Vec<_>, _>>()?;
    let result = heap.moments(heap.moment_companion(output.value())?)?;
    assert_eq!(result.model(), model);
    assert_eq!(result.source_ids(), sources);
    check(result.covariance(), &expected)
}
/// Run the existing typed bundle transaction with its unchanged coordinate descriptor.
fn convert<P: Scalar + 'static, Q: Scalar + 'static>(
    heap: &mut Heap,
    coordinates: CheckedShape,
    source: Handle<Value, P>,
) -> TestResult<PublishedBundle<Q>> {
    let limits = ExecutionLimits::default();
    let contract = PrecisionContract::new(coordinates, &Descriptor::ratio())?;
    let operation =
        OperationDraft::precision(vec![contract], P::PRECISION, Q::PRECISION).check(limits)?;
    let bound =
        Binding::<P, Q>::convert_precision(operation, vec![source], limits).validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    Ok(*outcome.output().first().ok_or("precision output")?)
}
