// SPDX-License-Identifier: AGPL-3.0-only
//! Affine reading and restoration preserve the original nominal target's numerical enclosure.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, contract, measure, owner, shape};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits, Role};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Heap, HeapLimits, OperationDraft, ValueBinding,
};
use nrz_scalar::Scalar;

/// Import errors survive normalization, an identity reading and physical restoration in both formats.
/// ´test:integration:affine-roundtrip-preserves-prior-numerical-error´
#[test]
fn affine_roundtrip_preserves_prior_numerical_error() -> TestResult {
    roundtrip::<f32>()?;
    roundtrip::<f64>()
}

/// Exercise the same imported nominal family through three consecutive public transactions.
fn roundtrip<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let source = Descriptor::ratio();
    let target = Descriptor::new(
        source.kind().clone(),
        measure(1, 2, 0, 1)?,
        Role::Difference,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let symbols = BindingSymbols::declare(
        &source,
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let original = heap.import_symbols(
        &symbols,
        &[P::from_u8(2)],
        Some(&[P::ONE / P::from_u8(2)]),
        Some(&[P::ONE / P::from_u8(8)]),
        Coverage::DeclaredBounds,
    )?;
    let outward = declarations.conversion_map(&source, &target)?;
    let identity = declarations.conversion_map(&target, &target)?;
    let source_shape = shape(&declarations, &source, 1)?;
    let target_shape = shape(&declarations, &target, 1)?;
    let mut current = original;
    for stage in 0..3 {
        let (input, output, map, scale) = match stage {
            0 => (&source_shape, &target_shape, outward.clone(), 2),
            1 => (&target_shape, &target_shape, identity.clone(), 2),
            _ => (&target_shape, &source_shape, outward.inverse()?, 1),
        };
        current = convert(
            &mut heap,
            current,
            contract(input.clone(), output.clone(), map)?,
        )?;
        check(&heap, &current, scale)?;
    }
    Ok(())
}

/// Publish the complete output while retaining its symbol-selected input dependency.
fn convert<P: Scalar + 'static>(
    heap: &mut Heap,
    input: ValueBinding<P>,
    conversion: nrz_matrix::ConversionContract,
) -> TestResult<ValueBinding<P>> {
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::convert(vec![conversion]).check(limits)?;
    let bound = Binding::from_symbols(operation, vec![input], limits).validate(heap)?;
    let result = bound
        .result_bindings()
        .first()
        .ok_or("planned result")?
        .clone();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    bound.publish(candidate)?;
    Ok(result)
}

/// Compare both coverages with independent rational endpoint boxes.
fn check<P: Scalar + 'static>(heap: &Heap, binding: &ValueBinding<P>, scale: i128) -> TestResult {
    let view = heap.value_by_symbol(binding.symbol(), binding)?;
    let center = view.region().get(0, 0)?.to_f64();
    let total = heap
        .companion(view.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    let numerical = heap
        .companion(view.evaluation().ok_or("error")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    assert!(covers(center, numerical, 15 * scale, 8)?);
    assert!(covers(center, numerical, 17 * scale, 8)?);
    assert!(covers(center, total, 3 * scale, 2)?);
    assert!(covers(center, total, 5 * scale, 2)?);
    // The total carries the original radius once, despite its overlapping numerical companion.
    let expected = if scale == 2 { 1.0 } else { 0.5 };
    assert!(total < expected * 1.001);
    assert!(numerical <= total);
    assert_eq!(view.coverage(), Coverage::DeclaredBounds);
    Ok(())
}
