// SPDX-License-Identifier: AGPL-3.0-only
//! Point origins, represented coverage and complete converted companion lifecycles.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, contract, execute, measure, reference_owner, shape};
use nrz_matrix::{Coverage, Extent, Heap, HeapLimits};
use nrz_units::{BaseDimension, Descriptor, Dimension, Kind, Role};

/// Normalization subtracts the declared point origin and restoration adds it only to centers.
/// ´test:integration:point-normalization-restoration-encloses-exact-origins´
#[test]
fn point_normalization_restoration_encloses_exact_origins() -> TestResult {
    let celsius = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?;
    let declarations = reference_owner(&celsius)?;
    let kelvin = celsius.coherent();
    let input_shape = shape(&declarations, &kelvin, 1)?;
    let normalized_shape = shape(&declarations, &Descriptor::ratio(), 1)?;
    let map = declarations.normalization_map(&kelvin, "reference", Some("origin"))?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(Extent::new(1, 1)?, &kelvin, &[300.0_f64], &[2.0])?;
    let normalized = execute(
        &mut heap,
        contract(input_shape.clone(), normalized_shape.clone(), map.clone())?,
        source.value(),
    )?;
    let center = heap.value(normalized.value())?.region().get(0, 0)?;
    let total = heap
        .companion(normalized.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    let error = heap
        .companion(normalized.evaluation().ok_or("evaluation")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(center, error, 537, 200)?);
    assert!(covers(center, total, 497, 200)?);
    assert!(covers(center, total, 577, 200)?);
    let restored = execute(
        &mut heap,
        contract(normalized_shape, input_shape, map.inverse()?)?,
        normalized.value(),
    )?;
    let restored_center = heap.value(restored.value())?.region().get(0, 0)?;
    let radius = heap
        .companion(restored.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(restored_center, radius, 298, 1)?);
    assert!(covers(restored_center, radius, 302, 1)?);
    assert_eq!(
        heap.value(restored.value())?.coverage(),
        Coverage::DeclaredBounds
    );
    heap.retire(restored.value())?;
    assert!(
        heap.companion(restored.evaluation().ok_or("evaluation")?)
            .is_err()
    );
    Ok(())
}

/// The Celsius example and its return trip include both actually executed conversion errors.
/// ´test:integration:celsius-conversion-roundtrip-encloses-rational-oracle´
#[test]
fn celsius_conversion_roundtrip_encloses_rational_oracle() -> TestResult {
    let celsius = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?;
    let declarations = reference_owner(&celsius)?;
    let kelvin = celsius.coherent();
    let coordinates = shape(&declarations, &kelvin, 1)?;
    let map = declarations.conversion_map(&kelvin, &celsius)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &kelvin, &[0.0_f64])?;
    let presented = execute(
        &mut heap,
        contract(coordinates.clone(), coordinates.clone(), map.clone())?,
        source.value(),
    )?;
    let version = heap.value(presented.value())?.stamp().version();
    heap.replace_represented(presented.value(), version, &[20.0_f64])?;
    let coherent = execute(
        &mut heap,
        contract(coordinates.clone(), coordinates.clone(), map.inverse()?)?,
        presented.value(),
    )?;
    let center = heap.value(coherent.value())?.region().get(0, 0)?;
    let error = heap
        .companion(coherent.evaluation().ok_or("evaluation")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(center, error, 5863, 20)?);
    assert_eq!(
        heap.value(coherent.value())?.coverage(),
        Coverage::RepresentedInputs
    );
    let returned = execute(
        &mut heap,
        contract(coordinates.clone(), coordinates, map)?,
        coherent.value(),
    )?;
    assert!(covers(
        heap.value(returned.value())?.region().get(0, 0)?,
        heap.companion(returned.bound().ok_or("total")?)?
            .region()
            .get(0, 0)?,
        20,
        1
    )?);
    Ok(())
}
