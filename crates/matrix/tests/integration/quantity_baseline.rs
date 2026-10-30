// SPDX-License-Identifier: AGPL-3.0-only
//! Quantity entry measurements and the existing semantic refusal boundaries.

use crate::conversion_testing::{TestResult, measure, owner, shape};
use nrz_bindings::{
    BaseDimension, BinaryOp, BindingSymbols, Budget, Context, CountDomain, Declaration, Descriptor,
    Dimension, Evidence, ExponentPolicy, Kind, Limits, Measure, Registry, Role, UnitError,
};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationDraft,
    OperationError, ShapeLimits,
};

/// A fresh checked scalar declaration reaches three versioned heap regions.
/// ´test:integration:quantity-declaration-baseline-measurement´
/// ´claim:matrix:quantity-declaration-baseline´ A checked declaration retains both radius channels at numerical import.
#[test]
fn quantity_declaration_baseline_measurement() -> TestResult {
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["input"],
    )?;
    builder.declare(
        "input".into(),
        Declaration::Input {
            descriptor: Descriptor::ratio(),
            nonzero: false,
            evidence: Evidence::default(),
        },
    )?;
    let declarations = builder.freeze()?;
    let checked = declarations.checked("input")?;
    declarations.validate_checked(checked)?;
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(checked.descriptor(), [1, 1], [true, true], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::new(3, 3, 2))?;
    let source = heap.import_symbols(
        &symbols,
        &[3.0_f64],
        Some(&[0.25]),
        Some(&[0.125]),
        Coverage::DeclaredBounds,
    )?;
    let value = heap.value_by_symbol(source.symbol(), &source)?;
    assert_eq!(value.region().get(0, 0)?, 3.0);
    assert_eq!(value.stamp().version().number(), 1);
    for handle in [value.bound(), value.evaluation()].into_iter().flatten() {
        assert_eq!(heap.companion(handle)?.target_stamp(), value.stamp());
    }
    let counts = symbols.value().cas()?.counts();
    assert_eq!(
        (
            budget.usage().work,
            counts.vertices,
            counts.edges,
            counts.payload_bytes
        ),
        (12_005, 8, 11, 4_299)
    );
    Ok(())
}

/// Fresh declaration stores enforce the measured cumulative Work boundary.
/// ´test:integration:quantity-declaration-work-boundary´
/// ´claim:matrix:quantity-declaration-work´ The scalar declaration succeeds at 12005 Work and refuses at 12004 without refunding work.
#[test]
fn quantity_declaration_work_boundary() -> TestResult {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    let mut exact = Budget::new(Limits {
        work: 12_005,
        ..Limits::default()
    })?;
    BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut exact)?;
    assert_eq!(exact.usage().work, 12_005);
    let mut short = Budget::new(Limits {
        work: 12_004,
        ..Limits::default()
    })?;
    let Err(nrz_bindings::AdmissionError::Algebra(fault)) =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut short)
    else {
        panic!("one-below declaration Work must refuse");
    };
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            limit: 12_004,
            ..
        }
    ));
    assert!(short.usage().work > 0);
    Ok(())
}

/// Complete import needs three region slots/elements and two immutable quantity schemas.
/// ´test:integration:quantity-import-allocation-boundaries´
/// ´claim:matrix:quantity-import-allocation´ Scalar import atomically enforces three slots, three elements and two schemas.
#[test]
fn quantity_import_allocation_boundaries() -> TestResult {
    for capacities in [(3, 3, 2), (2, 3, 2), (3, 2, 2), (3, 3, 1)] {
        let symbols = BindingSymbols::declare(
            &Descriptor::ratio(),
            [1, 1],
            [true, true],
            &mut Budget::new(Limits::default())?,
        )?;
        let mut heap = Heap::new(HeapLimits::new(capacities.0, capacities.1, capacities.2))?;
        let result = heap.import_symbols(
            &symbols,
            &[3.0_f64],
            Some(&[0.25]),
            Some(&[0.125]),
            Coverage::DeclaredBounds,
        );
        if capacities == (3, 3, 2) {
            let value = result?;
            assert_eq!(
                heap.value_by_symbol(value.symbol(), &value)?
                    .region()
                    .get(0, 0)?,
                3.0
            );
        } else {
            let Err(fault) = result else {
                panic!("one-below allocation capacity must refuse");
            };
            assert_eq!(fault, HeapError::Capacity);
            let single =
                heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[7.0_f64])?;
            assert_eq!(heap.value(single.value())?.region().get(0, 0)?, 7.0);
        }
    }
    Ok(())
}

/// Exact role admission precedes the numerical difference-only arithmetic boundary.
/// ´test:integration:quantity-affine-baseline-refusals´
/// ´claim:matrix:quantity-affine-baseline´ Bindings admits point subtraction while the existing sum kernel refuses point inputs.
#[test]
fn quantity_affine_baseline_refusals() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let point = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        Measure::coherent(),
        Role::Point,
    )?;
    assert_eq!(
        registry.binary(BinaryOp::Subtract, &point, &point)?,
        point.difference()
    );
    let Err(fault) = registry.binary(BinaryOp::Add, &point, &point) else {
        panic!("point addition must refuse");
    };
    assert_eq!(fault, UnitError::AffineRole);
    let declarations = owner()?;
    let tangent = shape(&declarations, &point, 1)?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::subtract(tangent.clone(), tangent, ShapeLimits::default())?
        .check(limits)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_represented(Extent::new(1, 1)?, &point, &[300.0_f64])?;
    let right = heap.import_represented(Extent::new(1, 1)?, &point, &[280.0_f64])?;
    let Err(fault) = Binding::new(operation.clone(), vec![left.value(), right.value()], limits)
        .validate(&mut heap)
    else {
        panic!("point inputs must refuse at the difference kernel boundary");
    };
    assert!(matches!(fault, OperationError::InputMismatch { slot: 0 }));
    let left = heap.import_represented(Extent::new(1, 1)?, &point.difference(), &[300.0_f64])?;
    let right = heap.import_represented(Extent::new(1, 1)?, &point.difference(), &[280.0_f64])?;
    let bound =
        Binding::new(operation, vec![left.value(), right.value()], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let result = outcome.output().first().ok_or("difference result")?;
    assert_eq!(heap.value(result.value())?.region().get(0, 0)?, 20.0);
    Ok(())
}

/// Explicit count ratios are semantic evidence without floating count admission.
/// ´test:integration:quantity-count-baseline-refusals´
/// ´claim:matrix:quantity-count-baseline´ Explicit count ratios retain kind checks and floating count imports refuse.
#[test]
fn quantity_count_baseline_refusals() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let count = Descriptor::new(
        Kind::Count("events".into(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert_eq!(registry.count_ratio(&count, &count)?, Descriptor::ratio());
    let Err(fault) = registry.binary(BinaryOp::Divide, &count, &count) else {
        panic!("implicit count division must refuse");
    };
    assert_eq!(fault, UnitError::CountDomain);
    let Err(fault) = registry.count_ratio(&count, &Descriptor::ratio()) else {
        panic!("a continuous ratio must not substitute for a count");
    };
    assert_eq!(fault, UnitError::KindMismatch);
    let mut heap = Heap::new(HeapLimits::default())?;
    let Err(fault) = heap.import_represented(Extent::new(1, 1)?, &count, &[3.0_f64]) else {
        panic!("floating count storage must refuse");
    };
    assert_eq!(fault, HeapError::CountStorage);
    Ok(())
}

/// Fresh stores enforce the measured complete declaration allocation ceilings.
/// ´test:integration:quantity-declaration-byte-boundaries´
/// ´claim:matrix:quantity-declaration-bytes´ Declaration admission requires 7389 staged bytes and 4299 retained graph bytes.
#[test]
fn quantity_declaration_byte_boundaries() -> TestResult {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    for (kind, boundaries) in [
        (BudgetKind::StagedPayloadBytes, [7_389, 7_388]),
        (BudgetKind::PayloadBytes, [4_299, 4_298]),
    ] {
        for (position, limit) in boundaries.into_iter().enumerate() {
            let mut limits = Limits::default();
            if kind == BudgetKind::StagedPayloadBytes {
                limits.staged_payload_bytes = limit;
            } else {
                limits.payload_bytes = limit;
            }
            let mut budget = Budget::new(limits)?;
            let result =
                BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut budget);
            if position == 0 {
                result?;
                assert_eq!(budget.usage().work, 12_005);
            } else {
                let Err(nrz_bindings::AdmissionError::Algebra(fault)) = result else {
                    panic!("one-below declaration allocation must refuse");
                };
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind)
                );
            }
        }
    }
    Ok(())
}

/// Existing widening drops the prior numerical-only channel despite retaining the total.
/// ´test:integration:quantity-precision-baseline-measurement´
/// ´claim:matrix:quantity-precision-baseline´ Existing widening retains the total but resets the separate numerical-error channel.
#[test]
fn quantity_precision_baseline_measurement() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let contract =
        nrz_matrix::PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
    let symbols = BindingSymbols::declare(
        &quantity,
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_symbols(
        &symbols,
        &[3.0_f32],
        Some(&[0.25]),
        Some(&[0.125]),
        Coverage::DeclaredBounds,
    )?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::precision(
        vec![contract],
        nrz_scalar::Precision::F32,
        nrz_scalar::Precision::F64,
    )
    .check(limits)?;
    let view = heap.value_by_symbol(source.symbol(), &source)?;
    let raw = heap
        .companion(view.bound().ok_or("source total")?)?
        .target();
    let bound =
        Binding::<f32, f64>::convert_precision(operation, vec![raw], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let result = outcome.output().first().ok_or("widened")?;
    assert_eq!(
        heap.companion(result.bound().ok_or("total")?)?
            .region()
            .get(0, 0)?,
        0.25
    );
    assert_eq!(
        heap.companion(result.evaluation().ok_or("error")?)?
            .region()
            .get(0, 0)?,
        0.0
    );
    Ok(())
}

/// Noncoherent point coordinates need the existing explicit conversion transaction.
/// ´test:integration:quantity-measure-baseline-refusals´
/// ´claim:matrix:quantity-measure-baseline´ Celsius conversion is checked exactly while direct noncoherent import refuses.
#[test]
fn quantity_measure_baseline_refusals() -> TestResult {
    let celsius = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?;
    let declarations = owner()?;
    declarations.conversion_map(&celsius, &celsius.coherent())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let Err(fault) = heap.import_represented(Extent::new(1, 1)?, &celsius, &[20.0_f64]) else {
        panic!("direct Celsius import must refuse");
    };
    assert_eq!(fault, HeapError::NonCoherentMeasure);
    Ok(())
}

/// A fresh shaped declaration measures the same checked routing with four coordinates.
/// ´test:integration:quantity-array-baseline-measurement´
/// ´claim:matrix:quantity-array-baseline´ Arrays retain their intrinsic three-channel region inventory.
#[test]
fn quantity_array_baseline_measurement() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&Descriptor::ratio(), [2, 2], [true, true], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::new(3, 12, 2))?;
    let value = heap.import_symbols(
        &symbols,
        &[1.0_f64, 2.0, 3.0, 4.0],
        Some(&[0.25, 0.5, 0.75, 1.0]),
        Some(&[0.125, 0.25, 0.375, 0.5]),
        Coverage::DeclaredBounds,
    )?;
    assert_eq!(
        heap.value_by_symbol(value.symbol(), &value)?
            .region()
            .extent(),
        Extent::new(2, 2)?
    );
    let counts = symbols.value().cas()?.counts();
    assert_eq!((budget.usage().work, counts.payload_bytes), (12_005, 4_299));
    let view = heap.value_by_symbol(value.symbol(), &value)?;
    assert_eq!(view.region().export()?, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(view.region().get(1, 1)?, 4.0);
    assert_eq!(
        heap.companion(view.bound().ok_or("array total")?)?
            .region()
            .export()?,
        [0.25, 0.5, 0.75, 1.0]
    );
    assert_eq!(
        heap.companion(view.evaluation().ok_or("array error")?)?
            .region()
            .export()?,
        [0.125, 0.25, 0.375, 0.5]
    );
    Ok(())
}

/// The existing sum reports its new rounding error separately from the complete total.
/// ´test:integration:quantity-sum-error-baseline-measurement´
/// ´claim:matrix:quantity-sum-error-baseline´ Existing sum error excludes prior numerical errors while its total still encloses them.
#[test]
fn quantity_sum_error_baseline_measurement() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let shape = shape(&declarations, &quantity, 1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    for center in [3.0_f64, 2.0] {
        let symbols = BindingSymbols::declare(
            &quantity,
            [1, 1],
            [true, true],
            &mut Budget::new(Limits::default())?,
        )?;
        inputs.push(heap.import_symbols(
            &symbols,
            &[center],
            Some(&[0.25]),
            Some(&[0.125]),
            Coverage::DeclaredBounds,
        )?);
    }
    let limits = ExecutionLimits::default();
    let operation =
        OperationDraft::add(shape.clone(), shape, ShapeLimits::default())?.check(limits)?;
    let bound = Binding::from_symbols(operation, inputs, limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let result = outcome.output().first().ok_or("sum")?;
    assert_eq!(heap.value(result.value())?.region().get(0, 0)?, 5.0);
    let total = heap
        .companion(result.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    let error = heap
        .companion(result.evaluation().ok_or("error")?)?
        .region()
        .get(0, 0)?;
    assert!(crate::conversion_oracle::covers(5.0, total, 9, 2)?);
    assert!(crate::conversion_oracle::covers(5.0, total, 11, 2)?);
    assert!(!crate::conversion_oracle::covers(5.0, error, 19, 4)?);
    assert!(!crate::conversion_oracle::covers(5.0, error, 21, 4)?);
    Ok(())
}
