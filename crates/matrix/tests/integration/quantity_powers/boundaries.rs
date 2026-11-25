// SPDX-License-Identifier: AGPL-3.0-only
//! Literal unary caller and provenance boundaries preserve every consumed scalar charge.

use super::{TestResult, Unary, bounded, evidence, execute, fixture};
use nrz_bindings::{Budget, Descriptor, ExponentPolicy, Limits};
use nrz_matrix::{
    Heap, HeapLimits, KernelError, OperationError, Quantity, QuantityContext, QuantityError,
};

/// A fresh scalar fixture may warm exactly the same output schema once before the measured call.
fn prepared(operation: Unary, warm: bool) -> TestResult<(QuantityContext, Heap, Quantity<f64>)> {
    let (context, [length, area, _]) = fixture(ExponentPolicy::Rational)?;
    let ratio = Descriptor::ratio();
    let (descriptor, center) = match operation {
        Unary::Power(2) => (&length, 3.0),
        Unary::Power(_) => (&ratio, 3.0),
        Unary::Root => (&area, 9.0),
    };
    let capacity = if warm { 9 } else { 6 };
    let mut heap = Heap::new(HeapLimits::new(capacity, capacity, 12))?;
    let input = bounded(&context, &mut heap, descriptor, center, 0.25, 0.125)?;
    if warm {
        execute(
            &context,
            &mut heap,
            &input,
            operation,
            &mut Budget::new(Limits::default())?,
        )?;
    }
    Ok((context, heap, input))
}

/// Every unary schedule pins cold and warm success and one-below Work in fresh stores.
/// ´test:integration:quantity-powers-work-boundaries´
/// ´claim:matrix:quantity-powers-work-boundaries´ Positive, negative and zero integer powers and positive roots preserve exact cold/warm caller Work, partial scalar charges and atomic refusal at one-below.
#[test]
fn quantity_powers_work_boundaries() -> TestResult {
    for (operation, warm, required, refused, scalar) in [
        (Unary::Power(2), false, 13_340, 13_212, (1_309, 1_181)),
        (Unary::Power(2), true, 10_348, 10_220, (1_309, 1_181)),
        (Unary::Power(-1), false, 12_841, 12_713, (1_261, 1_133)),
        (Unary::Power(-1), true, 9_825, 9_697, (1_261, 1_133)),
        (Unary::Power(0), false, 12_391, 12_263, (1_260, 1_132)),
        (Unary::Power(0), true, 9_374, 9_246, (1_260, 1_132)),
        (Unary::Root, false, 12_398, 12_270, (1_258, 1_130)),
        (Unary::Root, true, 9_408, 9_280, (1_258, 1_130)),
    ] {
        for admitted in [false, true] {
            work_case(operation, warm, (required, refused), scalar, admitted)?;
        }
    }
    Ok(())
}

/// Refusal leaves enough original heap capacity for the admitted retry and does not warm a schema.
fn work_case(
    operation: Unary,
    warm: bool,
    caller: (u64, u64),
    scalar: (u64, u64),
    admitted: bool,
) -> TestResult {
    let (context, mut heap, input) = prepared(operation, warm)?;
    let version = input.view(&heap)?.stamp();
    let before = heap.provenance_usage();
    let mut budget = Budget::new(Limits {
        work: if admitted { caller.0 } else { caller.0 - 1 },
        ..Limits::default()
    })?;
    let result = execute(&context, &mut heap, &input, operation, &mut budget);
    if admitted {
        let result = result?;
        assert_eq!(budget.usage().work, caller.0);
        assert_eq!(evidence(&heap, &result)?.usage().work, scalar.0);
    } else {
        let Err(QuantityError::Execution(failure)) = result else {
            panic!("one-below unary scalar Work must refuse");
        };
        let OperationError::Kernel(KernelError::Quantity {
            cause: nrz_scalar::KernelError::WorkLimit,
            work,
        }) = failure.cause()
        else {
            panic!("unary scalar WorkLimit required");
        };
        assert_eq!((*work, budget.usage().work), (scalar.1, caller.1));
        assert_eq!(heap.provenance_usage(), before);
        let mut retry = Budget::new(Limits {
            work: caller.0,
            ..Limits::default()
        })?;
        execute(&context, &mut heap, &input, operation, &mut retry)?;
        assert_eq!(retry.usage().work, caller.0);
    }
    assert_eq!(input.view(&heap)?.stamp(), version);
    Ok(())
}

/// Every unary certificate trace participates in the complete retained graph byte limit.
/// ´test:integration:quantity-powers-provenance-boundaries´
/// ´claim:matrix:quantity-powers-provenance-boundaries´ Unary recipes publish their measured retained bytes at N and refuse N-minus-one after retaining all spent scalar Work.
#[test]
fn quantity_powers_provenance_boundaries() -> TestResult {
    for (operation, required, work) in [
        (Unary::Power(2), 2_129, 13_340),
        (Unary::Power(-1), 2_129, 12_841),
        (Unary::Power(0), 2_129, 12_391),
        (Unary::Root, 2_129, 12_398),
    ] {
        for admitted in [false, true] {
            let (context, mut heap, input) = prepared(operation, false)?;
            heap.set_provenance_limits(nrz_matrix::ProvenanceLimits::new(
                1024,
                16_384,
                65_536,
                if admitted { required } else { required - 1 },
                1_048_576,
            ))?;
            let before = heap.provenance_usage();
            let mut budget = Budget::new(Limits::default())?;
            let result = execute(&context, &mut heap, &input, operation, &mut budget);
            assert_eq!(budget.usage().work, work);
            if admitted {
                result?;
                assert_eq!(heap.provenance_usage().bytes(), required);
            } else {
                let Err(QuantityError::Execution(failure)) = result else {
                    panic!("one-below unary retained certificate bytes must refuse");
                };
                assert!(matches!(failure.cause(), OperationError::ProvenanceLimit));
                assert_eq!(heap.provenance_usage(), before);
            }
            input.view(&heap)?;
        }
    }
    Ok(())
}

/// Partial domain schedules have literal boundaries distinct from complete successful arithmetic.
/// ´test:integration:quantity-powers-domain-work-boundaries´
/// ´claim:matrix:quantity-powers-domain-work-boundaries´ Root and nonpositive-power domain refusals consume their measured Work and one-below returns scalar `WorkLimit` without a publication prefix.
#[test]
fn quantity_powers_domain_work_boundaries() -> TestResult {
    for (operation, required, spent, scalar) in [
        (Unary::Power(-1), 12_128, 12_124, (548, 544)),
        (Unary::Power(0), 11_679, 11_675, (548, 544)),
        (Unary::Root, 11_684, 11_683, (544, 543)),
    ] {
        for admitted in [false, true] {
            let (context, [_, area, _]) = fixture(ExponentPolicy::Rational)?;
            let ratio = Descriptor::ratio();
            let (descriptor, total) = match operation {
                Unary::Root => (&area, 1.0),
                Unary::Power(_) => (&ratio, 2.0),
            };
            let mut heap = Heap::new(HeapLimits::default())?;
            let input = bounded(&context, &mut heap, descriptor, 1.0_f64, total, 0.0)?;
            let before = heap.provenance_usage();
            let mut budget = Budget::new(Limits {
                work: if admitted { required } else { required - 1 },
                ..Limits::default()
            })?;
            let Err(QuantityError::Execution(failure)) =
                execute(&context, &mut heap, &input, operation, &mut budget)
            else {
                panic!("unary original domain must refuse");
            };
            let OperationError::Kernel(KernelError::Quantity { cause, work }) = failure.cause()
            else {
                panic!("unary scalar domain or Work refusal required");
            };
            if admitted {
                let expected = match operation {
                    Unary::Root => nrz_scalar::KernelError::PositiveRoot,
                    Unary::Power(_) => nrz_scalar::KernelError::Arithmetic(
                        nrz_scalar::ArithmeticFailure::ZeroInDivisor,
                    ),
                };
                assert_eq!(*cause, expected);
                assert_eq!((*work, budget.usage().work), (scalar.0, required));
            } else {
                assert_eq!(*cause, nrz_scalar::KernelError::WorkLimit);
                assert_eq!((*work, budget.usage().work), (scalar.1, spent));
            }
            assert_eq!(heap.provenance_usage(), before);
            assert_eq!(input.view(&heap)?.region().get(0, 0)?, 1.0);
        }
    }
    Ok(())
}
