// SPDX-License-Identifier: AGPL-3.0-only
//! Quantity roots share checked nonunit scale judgments only after complete publication.

use super::{TestResult, bounded, fixture};
use nrz_bindings::readings::algebra::Rational;
use nrz_bindings::{Budget, Descriptor, ExponentPolicy, Limits, Measure, Role, Scale};
use nrz_matrix::{Heap, HeapLimits, Quantity, QuantityContext};

/// Build distinct exact measures of the already registered area kind.
fn scaled(area: &Descriptor, scale: i64) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        area.kind().clone(),
        Measure::new(
            "unit",
            Scale::Exact(Rational::from_ratio(
                scale,
                1,
                &mut Budget::new(Limits::default())?,
            )?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?)
}

/// Each boundary owns fresh stores and warms only the explicitly named scale.
fn prepared(prime: Option<i64>, scale: i64) -> TestResult<(QuantityContext, Heap, Quantity<f64>)> {
    let (context, [_, area, _]) = fixture(ExponentPolicy::Rational)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    if let Some(prime) = prime {
        let source = bounded(
            &context,
            &mut heap,
            &scaled(&area, prime)?,
            9.0_f64,
            0.25,
            0.125,
        )?;
        context.square_root(&mut heap, &source, &mut Budget::new(Limits::default())?)?;
    }
    let input = bounded(
        &context,
        &mut heap,
        &scaled(&area, scale)?,
        9.0_f64,
        0.25,
        0.125,
    )?;
    Ok((context, heap, input))
}

/// Complete quantity boundaries include the same exact preparation or retained lookup.
/// ´test:integration:quantity-root-reuse-work-boundaries´
/// ´claim:matrix:quantity-root-reuse-work-boundaries´ Nonunit roots preserve literal cold, same-scale warm and different-scale miss Work boundaries while refusals preserve the cache, source and heap.
#[test]
fn quantity_root_reuse_work_boundaries() -> TestResult {
    for (prime, scale, required) in [(None, 4, 29_694), (Some(4), 4, 9_564), (Some(4), 9, 29_822)] {
        for admitted in [false, true] {
            let (context, mut heap, input) = prepared(prime, scale)?;
            let before = heap.provenance_usage();
            let original = input.view(&heap)?.stamp();
            let mut budget = Budget::new(Limits {
                work: required - u64::from(!admitted),
                ..Limits::default()
            })?;
            let result = context.square_root(&mut heap, &input, &mut budget);
            if admitted {
                let result = result?;
                assert_eq!(budget.usage().work, required);
                assert_eq!(result.view(&heap)?.region().get(0, 0)?, 3.0);
                let expected = if scale == 4 { 2 } else { 3 };
                assert_eq!(
                    result.descriptor().measure().rational_scale()?,
                    &Rational::from_ratio(expected, 1, &mut Budget::new(Limits::default())?)?
                );
            } else {
                let Err(nrz_matrix::QuantityError::Execution(failure)) = result else {
                    panic!("one-below quantity root Work must refuse");
                };
                assert!(matches!(
                    failure.cause(),
                    nrz_matrix::OperationError::Kernel(nrz_matrix::KernelError::Quantity {
                        cause: nrz_scalar::KernelError::WorkLimit,
                        ..
                    })
                ));
                assert_eq!(budget.usage().work, required - 128);
                assert_eq!(heap.provenance_usage(), before);
                let mut retry = Budget::new(Limits {
                    work: required,
                    ..Limits::default()
                })?;
                context.square_root(&mut heap, &input, &mut retry)?;
                assert_eq!(retry.usage().work, required);
            }
            assert_eq!(input.view(&heap)?.stamp(), original);
        }
    }
    Ok(())
}
