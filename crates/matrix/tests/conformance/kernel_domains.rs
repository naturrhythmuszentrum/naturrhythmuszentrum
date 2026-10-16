// SPDX-License-Identifier: AGPL-3.0-only
//! Domain and resource refusals publish nothing and retain the typed numerical premise.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, output, owner, shape};
use nrz_matrix::{
    Binding, DomainReason, Elementwise, ExecutionLimits, Extent, Heap, HeapLimits, KernelError,
    OperationDraft, OperationError,
};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Full-box domain violations and finite-format square overflow remain distinct typed reasons.
/// ´test:integration:nonlinear-domains-refuse-atomically-in-both-formats´
#[test]
fn nonlinear_domains_refuse_atomically_in_both_formats() -> TestResult {
    domains::<f32>()?;
    domains::<f64>()
}

/// Every native kernel checks its full input box and the representability of its complete image.
fn domains<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    for (function, center, radius, reason) in [
        (
            Elementwise::Reciprocal,
            P::ONE,
            P::ONE,
            DomainReason::ZeroInDivisor,
        ),
        (
            Elementwise::SquareRoot,
            P::ONE,
            P::from_u8(2),
            DomainReason::NegativeRoot,
        ),
        (
            Elementwise::Square,
            P::max_finite(),
            P::ZERO,
            DomainReason::UnboundedImage,
        ),
        (
            Elementwise::Reciprocal,
            P::max_finite(),
            P::max_finite(),
            DomainReason::ZeroInDivisor,
        ),
        (
            Elementwise::SquareRoot,
            -P::max_finite(),
            P::max_finite(),
            DomainReason::NegativeRoot,
        ),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let input = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[center],
            &[radius],
        )?;
        let stamp = heap.value(input.value())?.stamp();
        let usage = heap.provenance_usage();
        let limits = ExecutionLimits::default();
        let operation =
            OperationDraft::map_elementwise(contract.clone(), contract.clone(), function)?
                .check(limits)?;
        let bound = Binding::new(operation, vec![input.value()], limits).validate(&mut heap)?;
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("domain admitted")?;
        assert!(
            matches!(failure.cause(), OperationError::Kernel(KernelError::Domain { function: actual, entry: 0, reason: cause }) if *actual == function && *cause == reason)
        );
        drop(bound);
        assert_eq!(heap.value(input.value())?.stamp(), stamp);
        assert_eq!(heap.provenance_usage(), usage);
    }
    Ok(())
}

/// The nonlinear phase is bounded immediately below, at and above its declared work count.
/// ´test:integration:nonlinear-work-boundaries-preserve-positive-results´
#[test]
fn nonlinear_work_boundaries_preserve_positive_results() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let draft = OperationDraft::map_elementwise(contract.clone(), contract, Elementwise::Square)?;
    for capacity in [11, 12, 13] {
        let limits = ExecutionLimits::new(1, capacity, capacity);
        let checked = draft.clone().check(limits);
        if capacity == 11 {
            assert!(matches!(checked, Err(OperationError::ResourceLimit)));
            continue;
        }
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let bound = Binding::new(checked?, vec![source.value()], limits).validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate)?;
        let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
        assert!(Rational::new(4, 1)?.covered(center, radius)?);
        assert!(Rational::new(4, 1)?.covered(center, error)?);
    }
    Ok(())
}
