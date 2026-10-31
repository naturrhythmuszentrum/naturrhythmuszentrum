// SPDX-License-Identifier: AGPL-3.0-only
//! Exact ratio materialization retains count origins and independently checked numerical error.

use super::quantity_counts::context;
use crate::{conversion_oracle::covers, conversion_testing::TestResult};
use nrz_bindings::{Budget, Descriptor, Limits};
use nrz_matrix::{
    CountQuantity, Coverage, ExactCountRatio, Heap, HeapError, HeapLimits, Quantity,
    QuantityContext, QuantityError,
};
use nrz_scalar::Scalar;

/// Prepare exact count operands on a ledger separate from the operation being measured.
fn ratio(
    context: &QuantityContext,
    numerator: u64,
    denominator: u64,
) -> TestResult<ExactCountRatio> {
    let mut setup = Budget::new(Limits::default())?;
    let three = context.count("three", u64::MAX, &mut setup)?;
    let one = three.quotient(3, u64::MAX, &mut setup)?;
    let left = one.scale(numerator, u64::MAX, &mut setup)?;
    let right = one.scale(denominator, u64::MAX, &mut setup)?;
    Ok(context.count_ratio(&left, &right, &mut setup)?)
}

/// Read all three scalar channels and authenticate their common target.
fn channels<P: Scalar + 'static>(quantity: &Quantity<P>, heap: &Heap) -> TestResult<[f64; 3]> {
    let view = quantity.view(heap)?;
    assert_eq!(quantity.descriptor(), &Descriptor::ratio());
    assert_eq!(view.coverage(), Coverage::DeclaredBounds);
    let total = heap.companion(view.bound().ok_or("complete total")?)?;
    let error = heap.companion(view.evaluation().ok_or("numerical error")?)?;
    assert_eq!(total.target_stamp(), view.stamp());
    assert_eq!(error.target_stamp(), view.stamp());
    Ok([
        view.region().get(0, 0)?.to_f64(),
        total.region().get(0, 0)?.to_f64(),
        error.region().get(0, 0)?.to_f64(),
    ])
}

/// Test an exact oracle independently of the production rational decoder and bound arithmetic.
fn oracle<P: Scalar + 'static>(numerator: u64, denominator: u64) -> TestResult<[f64; 3]> {
    let context = context()?;
    let source = ratio(&context, numerator, denominator)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let output = context.materialize_count_ratio::<P>(
        &mut heap,
        &source,
        &mut Budget::new(Limits::default())?,
    )?;
    let [center, total, error] = channels(&output, &heap)?;
    assert_eq!(total, error);
    assert!(covers(
        center,
        error,
        i128::from(numerator),
        i128::from(denominator)
    )?);
    let retained = output.count_ratio_sources().first().ok_or("exact origin")?;
    assert_eq!(retained.exact(), source.exact());
    assert_eq!(
        retained.operands().each_ref().map(CountQuantity::value),
        [numerator, denominator]
    );
    for operand in retained.operands() {
        assert!(
            matches!(operand.source().evidence(), nrz_bindings::CountEvidence::Declared { name, .. } if name == "three")
        );
    }
    Ok([center, total, error])
}

/// Both formats enclose nonrepresentable fractions and preserve exact representable cases.
/// ´test:integration:quantity-count-ratio-materialization-oracles´
/// ´claim:matrix:quantity-count-ratio-materialization-oracles´ Binary32 and binary64 ratio imports retain original count evidence and exact-oracle representation errors inside complete totals.
#[test]
fn quantity_count_ratio_materialization_oracles() -> TestResult {
    for (numerator, denominator) in [(0, 3), (3, 3), (3, 2), (3, 1)] {
        let wide = oracle::<f64>(numerator, denominator)?;
        let narrow = oracle::<f32>(numerator, denominator)?;
        assert_eq!(wide, narrow);
        assert_eq!(wide[1..], [0.0, 0.0]);
    }
    for (numerator, wide_bits, narrow_bits) in [
        (1, 0x3fd5_5555_5555_5554, 0x3eaa_aaaa),
        (2, 0x3fe5_5555_5555_5554, 0x3f2a_aaaa),
    ] {
        let wide = oracle::<f64>(numerator, 3)?;
        let narrow = oracle::<f32>(numerator, 3)?;
        assert_eq!(wide[0].to_bits(), wide_bits);
        assert_eq!(narrow[0], f64::from(f32::from_bits(narrow_bits)));
        assert!(wide[2] > 0.0);
        assert!(narrow[2] > wide[2]);
    }
    Ok(())
}

/// Count provenance survives optional roles, repeated precision conversion and arithmetic.
/// ´test:integration:quantity-count-ratio-materialization-lineage´
/// ´claim:matrix:quantity-count-ratio-materialization-lineage´ Quantity conversions and sums retain both exact ratio origins and the original nominal-target error.
#[test]
fn quantity_count_ratio_materialization_lineage() -> TestResult {
    let context = context()?;
    let source = ratio(&context, 1, 3)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = Budget::new(Limits::default())?;
    let wide = context.materialize_count_ratio::<f64>(&mut heap, &source, &mut budget)?;
    let lifted = wide.difference(&heap, &mut budget)?.dynamic();
    let narrow = context.narrow(&mut heap, &lifted, &mut budget)?;
    let restored = context.widen(&mut heap, &narrow, &mut budget)?;
    let converted = context.convert(&mut heap, &restored, &Descriptor::ratio(), &mut budget)?;
    for output in [&wide, &lifted, &restored, &converted] {
        let [center, _, error] = channels(output, &heap)?;
        assert!(covers(center, error, 1, 3)?);
        assert_eq!(output.count_ratio_sources().len(), 1);
        assert_eq!(output.count_ratio_sources()[0].exact(), source.exact());
    }
    let sum = context.add(&mut heap, &wide, &converted, &mut budget)?;
    let [center, total, error] = channels(&sum, &heap)?;
    assert!(covers(center, total, 2, 3)?);
    assert!(covers(center, error, 2, 3)?);
    assert_eq!(sum.count_ratio_sources().len(), 2);
    assert_eq!(sum.count_ratio_sources()[0].exact(), source.exact());
    assert_eq!(sum.count_ratio_sources()[1].exact(), source.exact());
    Ok(())
}

/// A complete import needs every value and companion slot, element and schema.
/// ´test:integration:quantity-count-ratio-materialization-capacity´
/// ´claim:matrix:quantity-count-ratio-materialization-capacity´ Ratio import refuses insufficient heap capacity atomically with source evidence and spent Work intact.
#[test]
fn quantity_count_ratio_materialization_capacity() -> TestResult {
    for capacities in [(3, 3, 2), (2, 3, 2), (3, 2, 2), (3, 3, 1)] {
        let context = context()?;
        let source = ratio(&context, 1, 3)?;
        let mut heap = Heap::new(HeapLimits::new(capacities.0, capacities.1, capacities.2))?;
        let mut budget = Budget::new(Limits::default())?;
        let result = context.materialize_count_ratio::<f64>(&mut heap, &source, &mut budget);
        if capacities == (3, 3, 2) {
            let [center, total, _] = channels(&result?, &heap)?;
            assert!(covers(center, total, 1, 3)?);
        } else {
            let Err(QuantityError::Heap(fault)) = result else {
                panic!("incomplete import capacity must refuse");
            };
            assert_eq!(fault, HeapError::Capacity);
            let value = heap.import_represented(
                nrz_matrix::Extent::new(1, 1)?,
                &Descriptor::ratio(),
                &[7.0_f64],
            )?;
            assert_eq!(heap.value(value.value())?.region().get(0, 0)?, 7.0);
        }
        assert!(budget.usage().work > 0);
        assert_eq!(
            source.operands().each_ref().map(CountQuantity::value),
            [1, 3]
        );
    }
    Ok(())
}

/// Inspect only the named caller Work refusal and its measured boundary.
fn work_refusal<P: Scalar>(error: &QuantityError<P>, limit: u64, needed: u64) -> TestResult {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    let QuantityError::Resources(fault) = error else {
        return Err("expected caller Work refusal".into());
    };
    let FaultKind::Exhausted {
        budget,
        limit: actual,
        needed: required,
        ..
    } = &fault.kind
    else {
        return Err("expected exhausted Work".into());
    };
    assert_eq!(*budget, BudgetKind::Work);
    assert_eq!([*actual, *required], [limit, needed]);
    Ok(())
}

/// Both formats exercise fresh sources and an otherwise empty three-region heap.
fn import_boundary<P: Scalar + 'static>() -> TestResult {
    for limit in [12_104, 12_103] {
        let context = context()?;
        let source = ratio(&context, 1, 3)?;
        let mut heap = Heap::new(HeapLimits::new(3, 3, 2))?;
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        let result = context.materialize_count_ratio::<P>(&mut heap, &source, &mut budget);
        if limit == 12_104 {
            let [center, total, _] = channels(&result?, &heap)?;
            assert!(covers(center, total, 1, 3)?);
            assert_eq!(budget.usage().work, 12_104);
        } else {
            let Err(fault) = result else {
                panic!("one-below import Work must refuse");
            };
            work_refusal(&fault, 12_103, 12_104)?;
            assert!(budget.usage().work > 0);
            assert!(budget.usage().work <= limit);
            context.materialize_count_ratio::<P>(
                &mut heap,
                &source,
                &mut Budget::new(Limits::default())?,
            )?;
        }
        assert_eq!(
            source.operands().each_ref().map(CountQuantity::value),
            [1, 3]
        );
    }
    Ok(())
}

/// Exact scalar import and source-retaining arithmetic have literal fresh-store boundaries.
/// ´test:integration:quantity-count-ratio-materialization-work´
/// ´claim:matrix:quantity-count-ratio-materialization-work´ Ratio imports require 12104 Work in either format and source-retaining sums require 12010, with atomic refusal one unit below.
#[test]
fn quantity_count_ratio_materialization_work() -> TestResult {
    import_boundary::<f32>()?;
    import_boundary::<f64>()?;
    for limit in [12_010, 12_009] {
        let context = context()?;
        let source = ratio(&context, 1, 3)?;
        let mut heap = Heap::new(HeapLimits::new(6, 6, 2))?;
        let input = context.materialize_count_ratio::<f64>(
            &mut heap,
            &source,
            &mut Budget::new(Limits::default())?,
        )?;
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        let result = context.add(&mut heap, &input, &input, &mut budget);
        if limit == 12_010 {
            assert_eq!(result?.count_ratio_sources().len(), 2);
            assert_eq!(budget.usage().work, 12_010);
        } else {
            let Err(fault) = result else {
                panic!("one-below sum Work must refuse");
            };
            work_refusal(&fault, 12_009, 12_010)?;
            assert!(budget.usage().work > 0);
            assert!(budget.usage().work <= limit);
            context.add(
                &mut heap,
                &input,
                &input,
                &mut Budget::new(Limits::default())?,
            )?;
        }
        let [center, total, _] = channels(&input, &heap)?;
        assert!(covers(center, total, 1, 3)?);
    }
    Ok(())
}

/// A numerical result cannot discard exact sources to fit a smaller metadata allowance.
/// ´test:integration:quantity-count-ratio-materialization-source-capacity´
/// ´claim:matrix:quantity-count-ratio-materialization-source-capacity´ Exhausted exact-source metadata refuses imports and sums before numerical publication while preserving original operands and spent Work.
#[test]
fn quantity_count_ratio_materialization_source_capacity() -> TestResult {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    for staged in [false, true] {
        let context = context()?;
        let source = ratio(&context, 1, 3)?;
        let mut heap = Heap::new(HeapLimits::new(6, 6, 2))?;
        let limits = if staged {
            Limits {
                staged_payload_bytes: 1,
                ..Limits::default()
            }
        } else {
            Limits {
                output_bytes: 1,
                ..Limits::default()
            }
        };
        let expected = if staged {
            BudgetKind::StagedPayloadBytes
        } else {
            BudgetKind::OutputBytes
        };
        let mut budget = Budget::new(limits.clone())?;
        let Err(QuantityError::Resources(fault)) =
            context.materialize_count_ratio::<f64>(&mut heap, &source, &mut budget)
        else {
            panic!("exact source metadata must fit before import");
        };
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == expected));
        assert!(budget.usage().work > 0);
        let input = context.materialize_count_ratio::<f64>(
            &mut heap,
            &source,
            &mut Budget::new(Limits::default())?,
        )?;
        let mut budget = Budget::new(limits)?;
        let Err(QuantityError::Resources(fault)) =
            context.add(&mut heap, &input, &input, &mut budget)
        else {
            panic!("both source inventories must fit before summation");
        };
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == expected));
        assert!(budget.usage().work > 0);
        let sum = context.add(
            &mut heap,
            &input,
            &input,
            &mut Budget::new(Limits::default())?,
        )?;
        assert_eq!(sum.count_ratio_sources().len(), 2);
        let [center, total, _] = channels(&input, &heap)?;
        assert!(covers(center, total, 1, 3)?);
        assert_eq!(
            source.operands().each_ref().map(CountQuantity::value),
            [1, 3]
        );
    }
    Ok(())
}
