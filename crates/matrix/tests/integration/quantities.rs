// SPDX-License-Identifier: AGPL-3.0-only
//! Quantity operations retain descriptor roles, original error channels and live companions.

mod costs;
mod refusals;
mod statistics;

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, measure, owner, reference_owner};
use nrz_bindings::{
    BaseDimension, BindingSymbols, Budget, Descriptor, Dimension, Kind, Limits, Measure, Role,
    UnitError,
};
use nrz_matrix::{
    Coverage, ExecutionLimits, Heap, HeapLimits, Quantity, QuantityContext, QuantityError,
    QuantityInput, QuantityRole, ShapeLimits,
};
use nrz_scalar::Scalar;

/// Retain a frozen registry and unchanged shape/execution policies.
fn context() -> TestResult<QuantityContext> {
    Ok(QuantityContext::new(
        owner()?,
        ShapeLimits::default(),
        ExecutionLimits::default(),
    ))
}

/// The independent exact Celsius definition includes its point role.
fn celsius() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?)
}

/// A fresh checked symbol is imported without losing its declared coordinate measure.
fn represented<P: Scalar + 'static>(
    context: &QuantityContext,
    heap: &mut Heap,
    descriptor: &Descriptor,
    center: P,
) -> TestResult<Quantity<P>> {
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(descriptor, [1, 1], [false, false], &mut budget)?;
    Ok(context.import(
        heap,
        &symbols,
        QuantityInput::represented(std::slice::from_ref(&center)),
        &mut budget,
    )?)
}

/// Read all three channels and validate that both companions target the actual result version.
fn channels<P: Scalar + 'static, R: QuantityRole>(
    heap: &Heap,
    quantity: &Quantity<P, R>,
) -> TestResult<[f64; 3]> {
    let view = quantity.view(heap)?;
    let total = heap.companion(view.bound().ok_or("total")?)?;
    let error = heap.companion(view.evaluation().ok_or("error")?)?;
    assert_eq!(total.target_stamp(), view.stamp());
    assert_eq!(error.target_stamp(), view.stamp());
    assert_eq!(total.target(), error.target());
    Ok([
        view.region().get(0, 0)?.to_f64(),
        total.region().get(0, 0)?.to_f64(),
        error.region().get(0, 0)?.to_f64(),
    ])
}

/// Point subtraction and translations use the bindings role table before sum execution.
/// ´test:integration:quantity-affine-arithmetic-retains-roles´
/// ´claim:matrix:quantity-affine-arithmetic´ Compatible points subtract to differences and translations preserve points while forbidden role pairs refuse.
#[test]
fn quantity_affine_arithmetic_retains_roles() -> TestResult {
    let context = context()?;
    let celsius = celsius()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = represented(&context, &mut heap, &celsius, 20.0_f64)?;
    let right = represented(&context, &mut heap, &celsius, 10.0_f64)?;
    let delta = represented(&context, &mut heap, &celsius.difference(), 2.0_f64)?;
    let mut budget = Budget::new(Limits::default())?;
    let left_point = left.point(&heap, &mut budget)?;
    let right_point = right.point(&heap, &mut budget)?;
    let difference = context.point_difference(&mut heap, &left_point, &right_point, &mut budget)?;
    assert_eq!(difference.descriptor(), &celsius.difference());
    assert!(covers(
        channels(&heap, &difference)?[0],
        channels(&heap, &difference)?[2],
        10,
        1
    )?);
    let delta_difference = delta.difference(&heap, &mut budget)?;
    let translated = context.translate(&mut heap, &left_point, &delta_difference, &mut budget)?;
    assert_eq!(translated.descriptor(), &celsius);
    assert!(covers(
        channels(&heap, &translated)?[0],
        channels(&heap, &translated)?[2],
        22,
        1
    )?);
    for (operation, expected) in [(false, 22), (true, 18)] {
        let result = if operation {
            context.subtract(&mut heap, &left, &delta, &mut budget)?
        } else {
            context.add(&mut heap, &delta, &left, &mut budget)?
        };
        let [center, _, error] = channels(&heap, &result)?;
        assert!(covers(center, error, expected, 1)?);
        assert_eq!(result.descriptor().role(), Role::Point);
    }
    let records = heap.provenance_usage();
    let Err(QuantityError::Units(fault)) = context.add(&mut heap, &left, &right, &mut budget)
    else {
        panic!("point plus point must refuse");
    };
    assert_eq!(fault, UnitError::AffineRole);
    let Err(QuantityError::Units(fault)) = context.subtract(&mut heap, &delta, &left, &mut budget)
    else {
        panic!("difference minus point must refuse");
    };
    assert_eq!(fault, UnitError::AffineRole);
    assert_eq!(heap.provenance_usage(), records);
    Ok(())
}

/// Exact Celsius/kelvin maps and their round trips retain all propagated radii in each precision.
/// ´test:integration:quantity-temperature-roundtrip-encloses-exact-oracle´
/// ´claim:matrix:quantity-temperature-roundtrip´ Celsius and kelvin quantity conversions enclose independent rational targets and preserve point roles.
#[test]
fn quantity_temperature_roundtrip_encloses_exact_oracle() -> TestResult {
    temperature(20.0_f32, 0.5_f32, 0.125_f32)?;
    temperature(20.0_f64, 0.5_f64, 0.125_f64)
}

/// Exercise one sealed scalar format without using production arithmetic for oracle values.
fn temperature<P: Scalar + 'static>(center: P, total: P, error: P) -> TestResult {
    let context = context()?;
    let celsius = celsius()?;
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&celsius, [1, 1], [true, true], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = context.import(
        &mut heap,
        &symbols,
        QuantityInput::bounded(
            std::slice::from_ref(&center),
            std::slice::from_ref(&total),
            Some(std::slice::from_ref(&error)),
        ),
        &mut budget,
    )?;
    let kelvin = context.convert(&mut heap, &input, &celsius.coherent(), &mut budget)?;
    let [center, total, error] = channels(&heap, &kelvin)?;
    assert!(covers(center, error, 5863, 20)?);
    assert!(covers(center, error, 11721, 40)?);
    assert!(covers(center, error, 11731, 40)?);
    assert!(covers(center, total, 5853, 20)?);
    assert!(covers(center, total, 5873, 20)?);
    assert_eq!(kelvin.descriptor().role(), Role::Point);
    let returned = context.convert(&mut heap, &kelvin, &celsius, &mut budget)?;
    let [center, total, error] = channels(&heap, &returned)?;
    assert!(covers(center, error, 159, 8)?);
    assert!(covers(center, error, 161, 8)?);
    assert!(covers(center, total, 39, 2)?);
    assert!(covers(center, total, 41, 2)?);
    assert_eq!(returned.view(&heap)?.coverage(), Coverage::DeclaredBounds);
    Ok(())
}

/// Origins and exact references stay attached to normalization and its inverse.
/// ´test:integration:quantity-origin-removal-restoration-retains-map´
/// ´claim:matrix:quantity-origin-map´ Normalization removes only its checked origin and restoration uses the identical inverse map.
#[test]
fn quantity_origin_removal_restoration_retains_map() -> TestResult {
    let celsius = celsius()?;
    let context = QuantityContext::new(
        reference_owner(&celsius)?,
        ShapeLimits::default(),
        ExecutionLimits::default(),
    );
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = represented(&context, &mut heap, &celsius, 20.0_f64)?;
    let mut budget = Budget::new(Limits::default())?;
    let normalized =
        context.normalize(&mut heap, &input, "reference", Some("origin"), &mut budget)?;
    assert_eq!(normalized.quantity().descriptor(), &Descriptor::ratio());
    let [center, _, error] = channels(&heap, normalized.quantity())?;
    assert!(covers(center, error, 2, 1)?);
    let restored = context.restore(&mut heap, &normalized, &mut budget)?;
    let [center, _, error] = channels(&heap, &restored)?;
    assert!(covers(center, error, 20, 1)?);
    assert_eq!(restored.descriptor(), &celsius);
    let Err(QuantityError::Operation(fault)) =
        context.normalize(&mut heap, &input, "reference", None, &mut budget)
    else {
        panic!("a point normalization must name its origin");
    };
    assert!(matches!(
        *fault,
        nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Map(
            nrz_bindings::MapError::Origin
        ))
    ));
    let foreign = QuantityContext::new(
        reference_owner(&celsius)?,
        ShapeLimits::default(),
        ExecutionLimits::default(),
    );
    let Err(QuantityError::Operation(fault)) = foreign.restore(&mut heap, &normalized, &mut budget)
    else {
        panic!("another declaration snapshot cannot restore this map");
    };
    assert!(matches!(
        *fault,
        nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Map(
            nrz_bindings::MapError::Snapshot(nrz_bindings::SnapshotError::ForeignContext)
        ))
    ));
    Ok(())
}

/// Every array element retains centimetre/metre scales and its own companion targets.
/// ´test:integration:quantity-length-array-conversion-encloses-oracle´
/// ´claim:matrix:quantity-length-array´ A four-entry centimetre array and its metre round trip retain every center and companion.
#[test]
fn quantity_length_array_conversion_encloses_oracle() -> TestResult {
    length_array::<f32>()?;
    length_array::<f64>()
}

/// Each format checks every coordinate and radius against independent rational truth.
fn length_array<P: Scalar + 'static>() -> TestResult {
    let context = context()?;
    let metre = Descriptor::new(
        Kind::Named(
            "length".into(),
            Box::new(Dimension::base(BaseDimension::Length)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let centimetre = Descriptor::new(
        metre.kind().clone(),
        measure(1, 100, 0, 1)?,
        Role::Difference,
    )?;
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&centimetre, [2, 2], [true, true], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = context.import(
        &mut heap,
        &symbols,
        QuantityInput::bounded(
            &[P::ONE, P::from_u8(25), P::from_u8(125), P::from_u8(250)],
            &[P::ONE / P::from_u8(2); 4],
            Some(&[P::ONE / P::from_u8(8); 4]),
        ),
        &mut budget,
    )?;
    let converted = context.convert(&mut heap, &input, &metre, &mut budget)?;
    let returned = context.convert(&mut heap, &converted, &centimetre, &mut budget)?;
    for (quantity, denominator) in [(&converted, 100), (&returned, 1)] {
        let view = quantity.view(&heap)?;
        let total = heap.companion(view.bound().ok_or("array total")?)?;
        let error = heap.companion(view.evaluation().ok_or("array error")?)?;
        assert_eq!(total.target_stamp(), view.stamp());
        assert_eq!(error.target_stamp(), view.stamp());
        for (index, nominal) in [1, 25, 125, 250].into_iter().enumerate() {
            let row = index % 2;
            let column = index / 2;
            let center = view.region().get(row, column)?.to_f64();
            let radius = total.region().get(row, column)?.to_f64();
            let numerical = error.region().get(row, column)?.to_f64();
            assert!(numerical <= radius);
            assert!(covers(center, numerical, nominal * 8 - 1, denominator * 8)?);
            assert!(covers(center, numerical, nominal * 8 + 1, denominator * 8)?);
            assert!(covers(center, radius, nominal * 2 - 1, denominator * 2)?);
            assert!(covers(center, radius, nominal * 2 + 1, denominator * 2)?);
        }
    }
    Ok(())
}

/// Narrowing and widening carry the original numerical error instead of resetting it.
/// ´test:integration:quantity-precision-roundtrip-retains-nominal-error´
/// ´claim:matrix:quantity-precision-roundtrip´ Precision conversion preserves both radius channels and adds representation error once.
#[test]
fn quantity_precision_roundtrip_retains_nominal_error() -> TestResult {
    let context = context()?;
    let quantity = Descriptor::ratio();
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&quantity, [1, 1], [true, true], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = context.import(
        &mut heap,
        &symbols,
        QuantityInput::bounded(&[0.1_f64], &[0.25], Some(&[0.125])),
        &mut budget,
    )?;
    let narrow = context.narrow(&mut heap, &input, &mut budget)?;
    let restored = context.widen(&mut heap, &narrow, &mut budget)?;
    let [center, total, error] = channels(&heap, &restored)?;
    assert!(covers(center, error, -1, 40)?);
    assert!(covers(center, error, 9, 40)?);
    assert!(covers(center, total, -3, 20)?);
    assert!(covers(center, total, 7, 20)?);
    assert!(error >= 0.125);
    assert!(total < 0.26);
    assert_eq!(channels(&heap, &narrow)?[2], error);
    assert_eq!(restored.descriptor(), &quantity);
    Ok(())
}

/// Foreign owners and changed value versions cannot be repaired by an equal descriptor.
/// ´test:integration:quantity-live-binding-refusals-retain-atomicity´
/// ´claim:matrix:quantity-live-binding´ Quantity access and arithmetic revalidate the original selected binding without publishing on refusal.
#[test]
fn quantity_live_binding_refusals_retain_atomicity() -> TestResult {
    let context = context()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let raw = heap.import_represented(
        nrz_matrix::Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[3.0_f64],
    )?;
    let binding = heap.symbol_binding(raw.value())?;
    let mut budget = Budget::new(Limits::default())?;
    let quantity = context.capture(&heap, binding, &mut budget)?;
    let original = quantity.view(&heap)?.stamp();
    let foreign = Heap::new(HeapLimits::default())?;
    let Err(fault) = quantity.view(&foreign) else {
        panic!("foreign quantity must refuse");
    };
    assert_eq!(fault, nrz_matrix::HeapError::ForeignHandle);
    heap.replace_represented(raw.value(), original.version(), &[3.0_f64])?;
    let Err(fault) = quantity.view(&heap) else {
        panic!("changed version must refuse");
    };
    assert_eq!(fault, nrz_matrix::HeapError::StaleEvidence);
    let records = heap.provenance_usage();
    let Err(QuantityError::Heap(fault)) = context.add(&mut heap, &quantity, &quantity, &mut budget)
    else {
        panic!("stale quantity arithmetic must refuse");
    };
    assert_eq!(fault, nrz_matrix::HeapError::StaleEvidence);
    assert_eq!(heap.provenance_usage(), records);
    assert_eq!(heap.value(raw.value())?.region().get(0, 0)?, 3.0);
    Ok(())
}
