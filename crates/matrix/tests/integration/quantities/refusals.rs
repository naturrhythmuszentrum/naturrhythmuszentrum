// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor and complete-companion refusals precede numerical publication.

use super::{TestResult, celsius, context, measure, represented};
use nrz_bindings::{
    BaseDimension, BindingSymbols, Budget, CountDomain, Descriptor, Dimension, Kind, Limits,
    Measure, Role, UnitError,
};
use nrz_matrix::{Heap, HeapError, HeapLimits, QuantityError, QuantityInput};

/// Equal or zero coordinates cannot erase incompatible original dimensions or kinds.
/// ´test:integration:quantity-original-operand-refusals-precede-floats´
/// ´claim:matrix:quantity-original-operands´ Quantity addition checks both original dimensions and kinds even for zero operands and publishes nothing on refusal.
#[test]
fn quantity_original_operand_refusals_precede_floats() -> TestResult {
    let context = context()?;
    let temperature = celsius()?.difference().coherent();
    let length = Descriptor::new(
        Kind::Named(
            "length".into(),
            Box::new(Dimension::base(BaseDimension::Length)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let other = Descriptor::new(
        Kind::Named(
            "other-temperature".into(),
            Box::new(temperature.dimension().clone()),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    for (descriptor, expected) in [
        (length, UnitError::DimensionMismatch),
        (other, UnitError::KindMismatch),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let left = represented(&context, &mut heap, &temperature, 0.0_f64)?;
        let right = represented(&context, &mut heap, &descriptor, 0.0_f64)?;
        let mut budget = Budget::new(Limits::default())?;
        context.add(&mut heap, &left, &left, &mut budget)?;
        let usage = heap.provenance_usage();
        let spent = budget.usage().work;
        let Err(QuantityError::Units(fault)) = context.add(&mut heap, &left, &right, &mut budget)
        else {
            panic!("wrong original descriptor must refuse");
        };
        assert_eq!(fault, expected);
        assert_eq!(budget.usage().work, spent + 1);
        assert_eq!(heap.provenance_usage(), usage);
        assert_eq!(left.view(&heap)?.region().get(0, 0)?, 0.0);
    }
    Ok(())
}

/// A common dimension and role do not authorize implicit coordinate conversion.
/// ´test:integration:quantity-common-measure-refusal-has-conversion-twin´
/// ´claim:matrix:quantity-common-measure´ Different scales or point origins refuse direct arithmetic and explicit conversion supplies an accepted twin.
#[test]
fn quantity_common_measure_refusal_has_conversion_twin() -> TestResult {
    let context = context()?;
    for point in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let descriptor = if point {
            celsius()?
        } else {
            Descriptor::ratio()
        };
        let alternate = if point {
            descriptor.coherent()
        } else {
            Descriptor::new(
                descriptor.kind().clone(),
                measure(1, 100, 0, 1)?,
                Role::Difference,
            )?
        };
        let left = represented(&context, &mut heap, &descriptor, 10.0_f64)?;
        let right = represented(&context, &mut heap, &alternate, 2.0_f64)?;
        let mut budget = Budget::new(Limits::default())?;
        let usage = heap.provenance_usage();
        let Err(QuantityError::CommonMeasure) =
            context.subtract(&mut heap, &left, &right, &mut budget)
        else {
            panic!("unequal coordinate measures must refuse");
        };
        assert_eq!(heap.provenance_usage(), usage);
        assert_eq!(budget.usage().work, 1);
        let converted = context.convert(&mut heap, &right, &descriptor, &mut budget)?;
        context.subtract(&mut heap, &left, &converted, &mut budget)?;
    }
    Ok(())
}

/// Imports preserve the full deterministic contract and keep discrete counts outside floating arenas.
/// ´test:integration:quantity-import-refusals-are-atomic´
/// ´claim:matrix:quantity-import-refusals´ Quantity imports refuse invalid radii, missing companions, nonfinite centers and floating counts without occupying result storage.
#[test]
fn quantity_import_refusals_are_atomic() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut budget)?;
    for (centers, total, error, expected) in [
        (
            [1.0_f64],
            [0.25],
            [0.5],
            HeapError::Symbols(nrz_bindings::AdmissionError::CompanionMismatch),
        ),
        ([1.0], [-0.25], [0.0], HeapError::InvalidBound { index: 0 }),
        ([f64::NAN], [0.25], [0.0], HeapError::NonFinite { index: 0 }),
    ] {
        let mut heap = Heap::new(HeapLimits::new(3, 3, 2))?;
        let Err(QuantityError::Heap(fault)) = context.import(
            &mut heap,
            &symbols,
            QuantityInput::bounded(&centers, &total, Some(&error)),
            &mut budget,
        ) else {
            panic!("invalid complete import must refuse");
        };
        assert_eq!(fault, expected);
        let value = context.import(
            &mut heap,
            &symbols,
            QuantityInput::bounded(&[1.0], &[0.25], Some(&[0.125])),
            &mut budget,
        )?;
        assert_eq!(value.view(&heap)?.region().get(0, 0)?, 1.0);
    }
    let mut heap = Heap::new(HeapLimits::new(3, 3, 2))?;
    let Err(QuantityError::Heap(HeapError::Symbols(
        nrz_bindings::AdmissionError::CompanionMismatch,
    ))) = context.import(
        &mut heap,
        &symbols,
        QuantityInput::bounded(&[1.0_f64], &[0.25], None),
        &mut budget,
    )
    else {
        panic!("missing declared error must refuse");
    };
    let count = Descriptor::new(
        Kind::Count("events".into(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let count_symbols = BindingSymbols::declare(&count, [1, 1], [false, false], &mut budget)?;
    let Err(QuantityError::Operation(fault)) = context.import(
        &mut heap,
        &count_symbols,
        QuantityInput::represented(&[f64::NAN]),
        &mut budget,
    ) else {
        panic!("floating count must refuse before its payload");
    };
    assert!(matches!(
        *fault,
        nrz_matrix::OperationError::Heap(HeapError::CountStorage)
    ));
    context.import(
        &mut heap,
        &symbols,
        QuantityInput::bounded(&[1.0_f64], &[0.25], Some(&[0.125])),
        &mut budget,
    )?;
    Ok(())
}

/// A late precision overflow keeps the spent ledger and releases every private output reservation.
/// ´test:integration:quantity-precision-overflow-is-atomic´
/// ´claim:matrix:quantity-precision-overflow´ Narrowing overflow retains its failure record, leaves the source unchanged and publishes no value or companion prefix.
#[test]
fn quantity_precision_overflow_is_atomic() -> TestResult {
    let context = context()?;
    for center in [1.0_f64, f64::MAX] {
        let mut heap = Heap::new(HeapLimits::new(4, 4, 2))?;
        let source = represented(&context, &mut heap, &Descriptor::ratio(), center)?;
        let stamp = source.view(&heap)?.stamp();
        let mut budget = Budget::new(Limits::default())?;
        let result = context.narrow(&mut heap, &source, &mut budget);
        if center.to_bits() == 1.0_f64.to_bits() {
            assert_eq!(result?.view(&heap)?.region().get(0, 0)?, 1.0_f32);
        } else {
            let Err(QuantityError::Execution(fault)) = result else {
                panic!("narrowing overflow must refuse");
            };
            assert!(matches!(
                fault.cause(),
                nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Arithmetic(
                    nrz_scalar::ArithmeticFailure::Overflow
                ))
            ));
            assert_eq!(fault.record().historical_evidence().len(), 0);
            assert!(budget.usage().work > 0);
            let spent = budget.usage().work;
            let symbols = BindingSymbols::declare(
                &Descriptor::ratio(),
                [1, 1],
                [true, true],
                &mut Budget::new(Limits::default())?,
            )?;
            context.import(
                &mut heap,
                &symbols,
                QuantityInput::bounded(&[1.0_f32], &[0.25], Some(&[0.125])),
                &mut budget,
            )?;
            assert!(budget.usage().work > spent);
        }
        assert_eq!(source.view(&heap)?.stamp(), stamp);
        assert_eq!(
            source.view(&heap)?.region().get(0, 0)?.to_bits(),
            center.to_bits()
        );
    }
    Ok(())
}

/// Published shape identities and frozen declaration authority remain part of quantity admission.
/// ´test:integration:quantity-published-axes-and-snapshots-remain-checked´
/// ´claim:matrix:quantity-published-shapes´ Repeated quantity operations reuse published axes while independently declared axes and foreign semantic snapshots refuse.
#[test]
fn quantity_published_axes_and_snapshots_remain_checked() -> TestResult {
    let context = context()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let ratio = Descriptor::ratio();
    let input = represented(&context, &mut heap, &ratio, 2.0_f64)?;
    let mut budget = Budget::new(Limits::default())?;
    let first = context.convert(&mut heap, &input, &ratio, &mut budget)?;
    let second = context.convert(&mut heap, &input, &ratio, &mut budget)?;
    context.add(&mut heap, &first, &first, &mut budget)?;
    let usage = heap.provenance_usage();
    let Err(QuantityError::Operation(fault)) = context.add(&mut heap, &first, &second, &mut budget)
    else {
        panic!("independently declared axes must refuse");
    };
    assert!(matches!(
        *fault,
        nrz_matrix::OperationError::Shape(nrz_matrix::ShapeError::AxisMismatch)
    ));
    assert_eq!(heap.provenance_usage(), usage);
    let foreign = super::context()?;
    let Err(QuantityError::Operation(fault)) = foreign.add(&mut heap, &first, &first, &mut budget)
    else {
        panic!("foreign frozen authority must refuse");
    };
    assert!(matches!(
        *fault,
        nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Map(
            nrz_bindings::MapError::Snapshot(nrz_bindings::SnapshotError::ForeignContext)
        ))
    ));
    assert_eq!(heap.provenance_usage(), usage);
    Ok(())
}

/// Numerical refusal cannot publish a new schema or evict an existing compatible authority.
/// ´test:integration:quantity-schema-publication-is-atomic´
/// ´claim:matrix:quantity-schema-publication´ A late narrowing refusal leaves a cold schema slot empty and preserves a previously published warm schema and all input versions.
#[test]
fn quantity_schema_publication_is_atomic() -> TestResult {
    let context = context()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let ordinary = represented(&context, &mut heap, &Descriptor::ratio(), 1.0_f64)?;
    let overflow = represented(&context, &mut heap, &Descriptor::ratio(), f64::MAX)?;
    let stamps = [ordinary.view(&heap)?.stamp(), overflow.view(&heap)?.stamp()];
    for expected in [11_150, 8_129] {
        let usage = heap.provenance_usage();
        let mut refused = Budget::new(Limits::default())?;
        let Err(QuantityError::Execution(fault)) =
            context.narrow(&mut heap, &overflow, &mut refused)
        else {
            panic!("overflow must refuse before schema publication");
        };
        assert!(matches!(
            fault.cause(),
            nrz_matrix::OperationError::Conversion(nrz_matrix::ConversionError::Arithmetic(
                nrz_scalar::ArithmeticFailure::Overflow
            ))
        ));
        assert_eq!(refused.usage().work, expected);
        assert_eq!(heap.provenance_usage(), usage);
        let mut accepted = Budget::new(Limits {
            work: expected,
            ..Limits::default()
        })?;
        let output = context.narrow(&mut heap, &ordinary, &mut accepted)?;
        assert_eq!(accepted.usage().work, expected);
        assert_eq!(output.view(&heap)?.region().get(0, 0)?, 1.0_f32);
        assert_eq!(
            [ordinary.view(&heap)?.stamp(), overflow.view(&heap)?.stamp()],
            stamps,
        );
    }
    Ok(())
}

/// A retained environment authenticates policy before the numerical transaction begins.
/// ´test:integration:quantity-retained-policy-is-checked´
/// ´claim:matrix:quantity-retained-policy´ A changed name-comparison policy refuses a warm quantity schema without changing the heap or evicting compatible authority.
#[test]
fn quantity_retained_policy_is_checked() -> TestResult {
    let context = context()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = represented(&context, &mut heap, &Descriptor::ratio(), 1.0_f64)?;
    let first = context.narrow(&mut heap, &input, &mut Budget::new(Limits::default())?)?;
    let usage = heap.provenance_usage();
    let stamp = first.view(&heap)?.stamp();
    let mut budget = Budget::new(Limits {
        name_bytes: 255,
        ..Limits::default()
    })?;
    let Err(QuantityError::Operation(error)) = context.narrow(&mut heap, &input, &mut budget)
    else {
        panic!("changed retained policy must refuse");
    };
    let nrz_matrix::OperationError::Heap(HeapError::Symbols(
        nrz_bindings::AdmissionError::Algebra(fault),
    )) = *error
    else {
        panic!("expected frozen environment policy refusal");
    };
    assert_eq!(
        fault.kind,
        nrz_bindings::readings::algebra::FaultKind::Invalid(
            nrz_bindings::readings::algebra::InvalidCode::Policy,
        ),
    );
    assert!(budget.usage().work > 0);
    assert_eq!(heap.provenance_usage(), usage);
    assert_eq!(first.view(&heap)?.stamp(), stamp);
    let second = context.narrow(&mut heap, &input, &mut Budget::new(Limits::default())?)?;
    assert_ne!(first.symbol(), second.symbol());
    Ok(())
}
