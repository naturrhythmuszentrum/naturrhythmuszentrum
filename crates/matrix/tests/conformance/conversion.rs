// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational numerical truth for publicly executed affine conversions.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, contract, execute, measure, owner, shape};
use nrz_bindings::{BaseDimension, Descriptor, Dimension, Kind, Role};
use nrz_matrix::{ConversionMode, Coverage, Extent, Heap, HeapLimits};
use nrz_scalar::Scalar;

/// The declared centimetre example has a total physical bound and a separate evaluation bound.
/// ´test:integration:measure-conversion-encloses-centimetre-oracle´
#[test]
fn measure_conversion_encloses_centimetre_oracle() -> TestResult {
    centimetres::<f32>()?;
    centimetres::<f64>()
}

/// Run the same exact problem in each supported format, without a production expected-value path.
fn centimetres<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let metre = Descriptor::new(
        Kind::Named(
            "length".into(),
            Box::new(Dimension::base(BaseDimension::Length)),
        ),
        nrz_bindings::Measure::coherent(),
        Role::Difference,
    )?;
    let centimetre = Descriptor::new(
        metre.kind().clone(),
        measure(1, 100, 0, 1)?,
        Role::Difference,
    )?;
    let source_shape = shape(&declarations, &metre, 1)?;
    let target_shape = shape(&declarations, &centimetre, 1)?;
    let map = declarations.conversion_map(&metre, &centimetre)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &metre, &[P::ONE])?;
    let presentation = execute(
        &mut heap,
        contract(source_shape.clone(), target_shape.clone(), map.clone())?,
        source.value(),
    )?;
    let version = heap.value(presentation.value())?.stamp().version();
    heap.replace_declared_bounds(
        presentation.value(),
        version,
        &[P::from_u8(125)],
        &[P::from_u8(2)],
    )?;
    let result = execute(
        &mut heap,
        contract(target_shape, source_shape, map.inverse()?)?,
        presentation.value(),
    )?;
    let value = heap.value(result.value())?;
    let center = value.region().get(0, 0)?.to_f64();
    let total = heap
        .companion(result.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    let error = heap
        .companion(result.evaluation().ok_or("error")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    assert!(covers(center, error, 5, 4)?);
    assert!(covers(center, total, 123, 100)?);
    assert!(covers(center, total, 127, 100)?);
    assert_eq!(value.coverage(), Coverage::DeclaredBounds);
    assert_ne!(result.bound(), result.evaluation());
    assert_eq!(
        heap.companion(result.evaluation().ok_or("error")?)?
            .target_stamp(),
        value.stamp()
    );
    Ok(())
}

/// Explicit statistical conversion requires concrete checked source coverage.
/// ´test:integration:statistical-conversion-refuses-a-missing-model´
#[test]
fn statistical_conversion_refuses_a_missing_model() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let coordinates = shape(&declarations, &quantity, 1)?;
    let map = declarations.conversion_map(&quantity, &quantity)?;
    let conversion = nrz_matrix::ConversionContract::new(
        coordinates.clone(),
        coordinates,
        map,
        nrz_scalar::LiteralLimits::default(),
        ConversionMode::Statistical,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
    assert!(matches!(
        execute(&mut heap, conversion, input.value())
            .err()
            .and_then(|error| error.downcast::<nrz_matrix::OperationError>().ok())
            .as_deref(),
        Some(nrz_matrix::OperationError::Statistics(
            nrz_matrix::StatisticsError::MissingSource { slot: 0, entry: 0 }
        ))
    ));
    Ok(())
}

/// A complete carried model enables the previously refused statistical conversion law.
/// ´test:integration:statistical-conversion-propagates-a-complete-model´
#[test]
fn statistical_conversion_propagates_a_complete_model() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let target = Descriptor::new(
        quantity.kind().clone(),
        measure(1, 10, 0, 1)?,
        Role::Difference,
    )?;
    let conversion = nrz_matrix::ConversionContract::new(
        shape(&declarations, &quantity, 1)?,
        shape(&declarations, &target, 1)?,
        declarations.conversion_map(&quantity, &target)?,
        nrz_scalar::LiteralLimits::default(),
        ConversionMode::Statistical,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let result = execute(&mut heap, conversion, input.value())?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::check(
        moments.covariance(),
        &[crate::kernel_oracle::Rational::new(25, 1)?],
    )
}

/// Missing point origins and nonconstant references preserve the bindings-owned typed refusals.
/// ´test:integration:conversion-refuses-missing-origins-and-references´
#[test]
fn conversion_refuses_missing_origins_and_references() -> TestResult {
    let point = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 0, 1)?,
        Role::Point,
    )?;
    let declarations = crate::conversion_testing::reference_owner(&point)?;
    assert!(matches!(
        declarations.normalization_map(&point, "reference", None),
        Err(nrz_bindings::MapError::Origin)
    ));
    assert!(matches!(
        declarations.normalization_map(&point.difference(), "missing", None),
        Err(nrz_bindings::MapError::ConstantDeclaration(_))
    ));
    assert!(matches!(
        nrz_bindings::uncertainty(
            nrz_bindings::UncertaintyMode::RelativeRadius,
            &point,
            None,
            None
        ),
        Err(nrz_bindings::UnitError::RelativeReference)
    ));
    Ok(())
}

/// The exact output scratch requirement succeeds at and above its limit, and refuses below it.
/// ´test:integration:conversion-scratch-limit-boundaries´
#[test]
fn conversion_scratch_limit_boundaries() -> TestResult {
    for capacity in [2, 3, 4] {
        let declarations = owner()?;
        let source = Descriptor::ratio();
        let target = Descriptor::new(
            source.kind().clone(),
            measure(100, 1, 0, 1)?,
            Role::Difference,
        )?;
        let map = declarations.conversion_map(&source, &target)?;
        let operation = nrz_matrix::OperationDraft::convert(vec![contract(
            shape(&declarations, &source, 1)?,
            shape(&declarations, &target, 1)?,
            map,
        )?])
        .check(nrz_matrix::ExecutionLimits::default())?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_represented(Extent::new(1, 1)?, &source, &[125.0_f64])?;
        let bound = nrz_matrix::Binding::new(
            operation,
            vec![source.value()],
            nrz_matrix::ExecutionLimits::new(1, capacity, capacity),
        )
        .validate(&mut heap);
        if capacity < 3 {
            assert!(matches!(
                bound,
                Err(nrz_matrix::OperationError::ResourceLimit)
            ));
        } else {
            let bound = bound?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let result = bound.publish(candidate)?.into_output();
            let output = result.first().ok_or("output")?;
            assert!(covers(
                heap.value(output.value())?.region().get(0, 0)?,
                heap.companion(output.evaluation().ok_or("evaluation")?)?
                    .region()
                    .get(0, 0)?,
                5,
                4
            )?);
        }
    }
    Ok(())
}

/// A mathematically incompatible output zero cannot be published merely because its units match.
/// ´test:integration:conversion-checks-output-structural-zeros´
#[test]
fn conversion_checks_output_structural_zeros() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let input_shape = shape(&declarations, &quantity, 1)?;
    let output_shape = nrz_matrix::ShapeDraft::new(
        input_shape.domain().clone(),
        input_shape.codomain().clone(),
        vec![nrz_matrix::BlockDraft::new(
            0..1,
            0..1,
            vec![nrz_matrix::EntryDraft::Zero(quantity.clone())],
        )],
        nrz_matrix::StructureScope::WholeFamily,
    )
    .check(
        std::rc::Rc::clone(&declarations),
        nrz_matrix::ShapeLimits::default(),
    )?;
    let map = declarations.conversion_map(&quantity, &quantity)?;
    let operation =
        nrz_matrix::OperationDraft::convert(vec![contract(input_shape, output_shape, map)?])
            .check(nrz_matrix::ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
    let bound = nrz_matrix::Binding::new(
        operation,
        vec![source.value()],
        nrz_matrix::ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let refusal = bound
        .publish(candidate)
        .err()
        .ok_or("false zero published")?;
    assert!(matches!(
        refusal.cause(),
        nrz_matrix::OperationError::StructureMismatch { slot: 0, entry: 0 }
    ));
    Ok(())
}

/// A computed bound does not supply physical coverage for untouched entries of a partial replacement.
/// ´test:integration:computed-bounds-cannot-promote-partial-physical-coverage´
#[test]
fn computed_bounds_cannot_promote_partial_physical_coverage() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let coordinates = shape(&declarations, &quantity, 2)?;
    let map = declarations.conversion_map(&quantity, &quantity)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(2, 1)?, &quantity, &[2.0_f64, 3.0])?;
    let output = execute(
        &mut heap,
        contract(coordinates.clone(), coordinates, map)?,
        source.value(),
    )?;
    let before = heap.value(output.value())?.stamp();
    let selected = heap.alias(
        output.value(),
        &nrz_matrix::Layout::gather(Extent::new(1, 1)?, 2, &[0], 1)?,
    )?;
    assert!(matches!(
        heap.replace_declared_bounds(selected, before.version(), &[2.0], &[0.25]),
        Err(nrz_matrix::HeapError::MissingCoverage)
    ));
    assert_eq!(heap.value(output.value())?.stamp(), before);
    assert_eq!(
        heap.value(output.value())?.coverage(),
        Coverage::RepresentedInputs
    );
    Ok(())
}
