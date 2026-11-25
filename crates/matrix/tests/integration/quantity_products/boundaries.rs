// SPDX-License-Identifier: AGPL-3.0-only
//! Literal complete-ledger boundaries retain scalar charges and publish no refusal prefix.

use super::{TestResult, bounded, evidence, fixture};
use nrz_bindings::{Budget, Limits};
use nrz_matrix::{Heap, HeapLimits, KernelError, OperationError, Quantity, QuantityError};

/// Cold and compatible warm operations have independent literal success and one-below stores.
/// ´test:integration:quantity-products-work-boundaries´
/// ´claim:matrix:quantity-products-work-boundaries´ Product and quotient caller Work is pinned for cold and warm publication; one-below scalar exhaustion retains spent Work and publishes no result prefix.
#[test]
fn quantity_products_work_boundaries() -> TestResult {
    for (divide, warm, required, refused) in [
        (false, false, 13_112, 12_984),
        (false, true, 10_132, 10_004),
        (true, false, 13_658, 13_530),
        (true, true, 10_680, 10_552),
    ] {
        for admitted in [false, true] {
            work_case(divide, warm, required, refused, admitted)?;
        }
    }
    Ok(())
}

/// Preserve the same physical fixture and optional warm-up in every fresh boundary store.
fn work_case(divide: bool, warm: bool, required: u64, refused: u64, admitted: bool) -> TestResult {
    let (context, [voltage, current, power]) = fixture()?;
    let capacity = if warm { 12 } else { 9 };
    let mut heap = Heap::new(HeapLimits::new(capacity, capacity, 12))?;
    let left = bounded(
        &context,
        &mut heap,
        if divide { &power } else { &voltage },
        6.0_f64,
        0.25,
        0.125,
    )?;
    let right = bounded(&context, &mut heap, &current, 2.0_f64, 0.25, 0.125)?;
    if warm {
        execute(
            &context,
            &mut heap,
            &left,
            &right,
            divide,
            &mut Budget::new(Limits::default())?,
        )?;
    }
    let before = heap.provenance_usage();
    let versions = [left.view(&heap)?.stamp(), right.view(&heap)?.stamp()];
    let mut budget = Budget::new(Limits {
        work: if admitted { required } else { required - 1 },
        ..Limits::default()
    })?;
    let result = execute(&context, &mut heap, &left, &right, divide, &mut budget);
    if admitted {
        let result = result?;
        assert_eq!(budget.usage().work, required);
        let scalar = evidence(&heap, &result)?.usage();
        assert_eq!(
            (scalar.work, scalar.exported),
            if divide { (2_197, 12) } else { (1_681, 9) }
        );
    } else {
        let Err(QuantityError::Execution(failure)) = result else {
            panic!("one-below scalar schedule Work must refuse");
        };
        let OperationError::Kernel(KernelError::Quantity {
            cause: nrz_scalar::KernelError::WorkLimit,
            work,
        }) = failure.cause()
        else {
            panic!("quantity scalar WorkLimit required");
        };
        assert_eq!(*work, if divide { 2_069 } else { 1_553 });
        assert_eq!(budget.usage().work, refused);
        assert_eq!(heap.provenance_usage(), before);
        let mut retry = Budget::new(Limits {
            work: required,
            ..Limits::default()
        })?;
        execute(&context, &mut heap, &left, &right, divide, &mut retry)?;
        assert_eq!(retry.usage().work, required);
    }
    assert_eq!(
        [left.view(&heap)?.stamp(), right.view(&heap)?.stamp()],
        versions
    );
    Ok(())
}

/// Both arithmetic laws share the same exact physical fixture and publication path.
fn execute(
    context: &nrz_matrix::QuantityContext,
    heap: &mut Heap,
    left: &Quantity<f64>,
    right: &Quantity<f64>,
    divide: bool,
    budget: &mut Budget,
) -> Result<Quantity<f64>, QuantityError<f64>> {
    if divide {
        context.divide(heap, left, right, budget)
    } else {
        context.multiply(heap, left, right, budget)
    }
}

/// The complete boxed certificate payload participates in retained provenance admission.
/// ´test:integration:quantity-products-provenance-boundaries´
/// ´claim:matrix:quantity-products-provenance-boundaries´ Product and quotient retain their measured certificate bytes at N and refuse N-minus-one after scalar execution without publishing heap records.
#[test]
fn quantity_products_provenance_boundaries() -> TestResult {
    for (divide, required, work) in [(false, 2_873, 13_112), (true, 3_185, 13_658)] {
        for admitted in [false, true] {
            let (context, [voltage, current, power]) = fixture()?;
            let mut heap = Heap::new(HeapLimits::default())?;
            let left = bounded(
                &context,
                &mut heap,
                if divide { &power } else { &voltage },
                6.0_f64,
                0.25,
                0.125,
            )?;
            let right = bounded(&context, &mut heap, &current, 2.0_f64, 0.25, 0.125)?;
            heap.set_provenance_limits(nrz_matrix::ProvenanceLimits::new(
                1024,
                16_384,
                65_536,
                if admitted { required } else { required - 1 },
                1_048_576,
            ))?;
            let before = heap.provenance_usage();
            let mut budget = Budget::new(Limits::default())?;
            let result = execute(&context, &mut heap, &left, &right, divide, &mut budget);
            assert_eq!(budget.usage().work, work);
            if admitted {
                result?;
                assert_eq!(heap.provenance_usage().bytes(), required);
            } else {
                let Err(QuantityError::Execution(failure)) = result else {
                    panic!("one-below retained quantity certificate bytes must refuse");
                };
                assert!(matches!(failure.cause(), OperationError::ProvenanceLimit));
                assert_eq!(heap.provenance_usage(), before);
            }
            assert_eq!(left.view(&heap)?.region().get(0, 0)?, 6.0);
            assert_eq!(right.view(&heap)?.region().get(0, 0)?, 2.0);
        }
    }
    Ok(())
}

/// Domain failure has its own complete Work limit and is distinct from schedule exhaustion.
/// ´test:integration:quantity-quotient-domain-work-boundary´
/// ´claim:matrix:quantity-quotient-domain-work-boundary´ The zero-containing divisor domain refusal consumes its literal Work total; one-below returns scalar `WorkLimit`, preserves the actual partial charge and publishes no prefix.
#[test]
fn quantity_quotient_domain_work_boundary() -> TestResult {
    for admitted in [false, true] {
        let (context, _) = fixture()?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let ratio = nrz_bindings::Descriptor::ratio();
        let left = bounded(&context, &mut heap, &ratio, 3.0_f64, 0.0, 0.0)?;
        let right = bounded(&context, &mut heap, &ratio, 1.0_f64, 2.0, 0.125)?;
        let before = heap.provenance_usage();
        let mut budget = Budget::new(Limits {
            work: if admitted { 12_561 } else { 12_560 },
            ..Limits::default()
        })?;
        let Err(QuantityError::Execution(failure)) =
            context.divide(&mut heap, &left, &right, &mut budget)
        else {
            panic!("zero-containing quotient must refuse without output");
        };
        let OperationError::Kernel(KernelError::Quantity { cause, work }) = failure.cause() else {
            panic!("scalar refusal and consumed Work required");
        };
        if admitted {
            assert_eq!(
                *cause,
                nrz_scalar::KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::ZeroInDivisor)
            );
            assert_eq!((*work, budget.usage().work), (1_108, 12_561));
        } else {
            assert_eq!(*cause, nrz_scalar::KernelError::WorkLimit);
            assert_eq!((*work, budget.usage().work), (1_104, 12_557));
        }
        assert_eq!(heap.provenance_usage(), before);
        assert_eq!(left.view(&heap)?.region().get(0, 0)?, 3.0);
        assert_eq!(right.view(&heap)?.region().get(0, 0)?, 1.0);
    }
    Ok(())
}
