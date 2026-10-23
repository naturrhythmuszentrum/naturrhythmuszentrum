// SPDX-License-Identifier: AGPL-3.0-only
//! Kernel integration consumes prior conversion totals without counting their evaluation twice.

use crate::conversion_testing::{contract as conversion, measure};
use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use crate::{Extent, Heap, HeapLimits, OperationDraft, ShapeLimits};
use nrz_bindings::{Descriptor, Role};

/// The total already covers the input evaluation region; adding a zero operand must not add it again.
/// ´test:crate:sum-consumes-conversion-total-exactly-once´
#[test]
fn sum_consumes_conversion_total_exactly_once() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let ratio = Descriptor::ratio();
    let third = Descriptor::new(ratio.kind().clone(), measure(1, 3, 0, 1)?, Role::Difference)?;
    let source_shape = shape(&declarations, &coordinate, &coordinate)?;
    let destination = source_shape.clone();
    let map = declarations.conversion_map(&ratio, &third)?;
    let convert = conversion(source_shape.clone(), destination.clone(), map)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &ratio, &[2.0_f64])?;
    let converted = execute(
        &mut heap,
        OperationDraft::convert(vec![convert]),
        vec![source.value()],
    )?;
    let reverse = declarations.conversion_map(&third, &ratio)?;
    let restored = execute(
        &mut heap,
        OperationDraft::convert(vec![conversion(
            destination,
            source_shape.clone(),
            reverse,
        )?]),
        vec![output(&converted)?.value()],
    )?;
    let prior = entry(&heap, output(&restored)?, 0)?;
    let zero = heap.import_represented(Extent::new(1, 1)?, &ratio, &[0.0_f64])?;
    let summed = execute(
        &mut heap,
        OperationDraft::add(source_shape.clone(), source_shape, ShapeLimits::default())?,
        vec![output(&restored)?.value(), zero.value()],
    )?;
    let actual = entry(&heap, output(&summed)?, 0)?;
    assert_eq!(actual.1.to_bits(), prior.1.to_bits());
    assert_eq!(actual.2, 0.0);
    let target = Rational::new(6, 1)?
        .divide(Rational::new(3, 1)?)?
        .multiply(Rational::new(1, 1)?)?;
    assert!(target.covered(actual.0, actual.1)?);
    assert!(Rational::binary(prior.0)?.covered(actual.0, actual.2)?);
    Ok(())
}

/// Numerical contracts recheck every operand and result extent independently of descriptor evidence.
/// ´test:crate:kernel-contract-rechecks-every-numerical-extent´
#[test]
fn kernel_contract_rechecks_every_numerical_extent() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let physical = shape(&declarations, &coordinate, &coordinate)?;
    let draft = OperationDraft::add(physical.clone(), physical, ShapeLimits::default())?;
    for position in 0..3 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let limits = crate::ExecutionLimits::default();
        let mut bound = crate::Binding::new(
            draft.clone().check(limits)?,
            vec![source.value(); 2],
            limits,
        )
        .validate(&mut heap)?;
        let crate::binding::NativeRecipe::Kernel(contract) = &mut bound.operation.recipe else {
            return Err("missing numerical kernel".into());
        };
        let changed = Extent::new(2, 1)?;
        if position < 2 {
            std::rc::Rc::make_mut(&mut contract.inputs)[position] = changed;
        } else {
            contract.output = changed;
        }
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("changed numerical extent admitted")?;
        assert!(matches!(
            failure.cause(),
            crate::OperationError::Kernel(crate::KernelError::Contract)
        ));
        drop(bound);
        assert_eq!(heap.descriptors.iter().count(), 1);
        assert_eq!(heap.value(source.value())?.region().get(0, 0)?, 2.0);
    }
    Ok(())
}
