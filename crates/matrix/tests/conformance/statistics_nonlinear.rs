// SPDX-License-Identifier: AGPL-3.0-only
//! Native derivative domain boundaries and numerical parameter errors.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::{attach, check};
use nrz_matrix::{
    Binding, Elementwise, ExecutionLimits, Extent, Heap, HeapLimits, MomentAssurance,
    OperationDraft, OperationError, StatisticsError,
};
use nrz_units::Descriptor;

/// Reciprocal and positive-root derivatives use certified coefficient enclosures in both formats.
/// ´test:integration:native-derivative-errors-enclose-exact-rational-coefficients´
#[test]
fn native_derivative_errors_enclose_exact_rational_coefficients() -> TestResult {
    native::<f32>()?;
    native::<f64>()
}
/// Check exact coefficients through the public parameter/error relation.
fn native<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    for (function, value, derivative, variance) in [
        (Elementwise::Reciprocal, 3, (-1, 9), (1, 324)),
        (Elementwise::SquareRoot, 4, (1, 4), (1, 64)),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_represented(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[P::from_u8(value)],
        )?;
        attach(&mut heap, source.value(), &[P::ONE / P::from_u8(4)], false)?;
        let result = output(&execute(
            &mut heap,
            OperationDraft::map_elementwise(input.clone(), input.clone(), function)?,
            vec![source.value()],
        )?)?;
        let moments = heap.moments(heap.moment_companion(result.value())?)?;
        assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
        check(
            moments.jacobian().ok_or("derivative")?,
            &[Rational::new(derivative.0, derivative.1)?],
        )?;
        check(
            moments.covariance(),
            &[Rational::new(variance.0, variance.1)?],
        )?;
    }
    Ok(())
}

/// A deterministic nonnegative square-root box can touch zero while no smooth statistical neighborhood exists.
/// ´test:integration:statistical-root-refuses-zero-and-nonsmooth-neighborhoods´
#[test]
fn statistical_root_refuses_zero_and_nonsmooth_neighborhoods() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    for (center, radius) in [(0.0_f64, 0.0), (1.0, 1.0)] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[center],
            &[radius],
        )?;
        attach(&mut heap, source.value(), &[0.25], false)?;
        let operation =
            OperationDraft::map_elementwise(input.clone(), input.clone(), Elementwise::SquareRoot)?
                .check(ExecutionLimits::default())?;
        let bound = Binding::new(operation, vec![source.value()], ExecutionLimits::default())
            .validate(&mut heap)?;
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("nonsmooth root admitted")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Statistics(StatisticsError::Domain {
                function: Elementwise::SquareRoot,
                entry: 0
            }) | OperationError::Kernel(nrz_matrix::KernelError::Domain {
                function: Elementwise::SquareRoot,
                entry: 0,
                ..
            })
        ));
        drop(bound);
        assert_eq!(heap.provenance_usage().records(), 0);
        assert!(heap.value(source.value()).is_ok());
    }
    Ok(())
}

/// Squaring independent rational endpoints validates an irrational root derivative without a floating oracle.
/// ´test:integration:statistical-irrational-root-derivative-has-a-certified-parameter-error´
#[test]
fn statistical_irrational_root_derivative_has_a_certified_parameter_error() -> TestResult {
    irrational_root::<f32>()?;
    irrational_root::<f64>()
}
/// For c = 2 and U = 1/4, J squared is exactly 1/8 and the local variance is exactly 1/32.
fn irrational_root<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::from_u8(2)])?;
    attach(&mut heap, source.value(), &[P::ONE / P::from_u8(4)], false)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::map_elementwise(contract.clone(), contract, Elementwise::SquareRoot)?,
        vec![source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    let jacobian = view.jacobian().ok_or("root derivative")?;
    assert!(crate::statistics_oracle::squared_bracket(
        Rational::new(1, 8)?,
        jacobian.get(0, 0)?.to_f64(),
        jacobian.error(0, 0)?.to_f64()
    )?);
    check(view.covariance(), &[Rational::new(1, 32)?])
}
