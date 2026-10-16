// SPDX-License-Identifier: AGPL-3.0-only
//! Native nonlinear enclosures use exact rational endpoint images and squared-root comparisons.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_matrix::{Elementwise, Extent, Heap, HeapLimits, OperationDraft};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Squaring retains the nonlinear radius and includes zero when the input box crosses it.
/// ´test:integration:native-square-encloses-nonlinear-endpoints´
#[test]
fn native_square_encloses_nonlinear_endpoints() -> TestResult {
    square_format::<f32>()?;
    square_format::<f64>()
}

/// Check a positive square and a zero-crossing square in one format.
fn square_format<P: Scalar + 'static>() -> TestResult {
    for (numerator, denominator, radius_numerator, radius_denominator) in
        [(2, 1, 1, 8), (1, 4, 1, 2)]
    {
        let center = P::from_u8(numerator) / P::from_u8(denominator);
        let radius = P::from_u8(radius_numerator) / P::from_u8(radius_denominator);
        let (candidate, bound, error) = run(Elementwise::Square, center, radius)?;
        let exact = Rational::new(i128::from(numerator), i128::from(denominator))?;
        let extent = Rational::new(i128::from(radius_numerator), i128::from(radius_denominator))?;
        assert!(exact.multiply(exact)?.covered(candidate, error)?);
        for endpoint in [exact.subtract(extent)?, exact.add(extent)?] {
            assert!(endpoint.multiply(endpoint)?.covered(candidate, bound)?);
        }
        if numerator == 1 {
            assert!(Rational::new(0, 1)?.covered(candidate, bound)?);
        }
    }
    Ok(())
}

/// Reciprocal verifies both signs on the complete interval, rather than using only its center.
/// ´test:integration:native-reciprocal-encloses-exact-rational-endpoints´
#[test]
fn native_reciprocal_encloses_exact_rational_endpoints() -> TestResult {
    reciprocal_format::<f32>()?;
    reciprocal_format::<f64>()
}

/// Compare the two reciprocal endpoint images and the nominal exact reciprocal.
fn reciprocal_format<P: Scalar + 'static>() -> TestResult {
    for negative in [false, true] {
        let center = if negative {
            -P::from_u8(2)
        } else {
            P::from_u8(2)
        };
        let (candidate, radius, error) =
            run(Elementwise::Reciprocal, center, P::ONE / P::from_u8(4))?;
        let sign = if negative { -1 } else { 1 };
        assert!(Rational::new(sign, 2)?.covered(candidate, error)?);
        for denominator in [7, 9] {
            assert!(Rational::new(4 * sign, denominator)?.covered(candidate, radius)?);
        }
    }
    Ok(())
}

/// Irrational square-root truth is checked by squaring rational endpoints inside the returned band.
/// ´test:integration:native-square-root-encloses-domain-and-zero´
#[test]
fn native_square_root_encloses_domain_and_zero() -> TestResult {
    root_format::<f32>()?;
    root_format::<f64>()
}

/// Positive, boundary-touching and singleton-zero input intervals all have deterministic root coverage.
fn root_format<P: Scalar + 'static>() -> TestResult {
    let (candidate, radius, error) = run(
        Elementwise::SquareRoot,
        P::from_u8(2),
        P::ONE / P::from_u8(4),
    )?;
    assert!(root_covers(
        candidate,
        error,
        Rational::new(2, 1)?,
        Rational::new(2, 1)?
    )?);
    assert!(root_covers(
        candidate,
        radius,
        Rational::new(7, 4)?,
        Rational::new(9, 4)?
    )?);
    let (candidate, radius, error) = run(Elementwise::SquareRoot, P::ONE, P::ONE)?;
    assert!(root_covers(
        candidate,
        radius,
        Rational::new(0, 1)?,
        Rational::new(2, 1)?
    )?);
    assert!(Rational::new(1, 1)?.covered(candidate, error)?);
    let (candidate, radius, error) = run(Elementwise::SquareRoot, P::ZERO, P::ZERO)?;
    assert!(Rational::new(0, 1)?.covered(candidate, radius)?);
    assert_eq!((candidate, radius, error), (0.0, 0.0, 0.0));
    Ok(())
}

/// Use inward rounded dyadic endpoints so bounded integer squaring remains an independent sufficient check.
fn root_covers(center: f64, radius: f64, lower: Rational, upper: Rational) -> TestResult<bool> {
    let center = Rational::binary(center)?;
    let radius = Rational::binary(radius)?;
    let left = center.subtract(radius)?.dyadic_inward(true)?;
    let right = center.add(radius)?.dyadic_inward(false)?;
    let zero = Rational::new(0, 1)?;
    Ok(
        (left.at_most(zero)? || left.multiply(left)?.at_most(lower)?)
            && zero.at_most(right)?
            && upper.at_most(right.multiply(right)?)?,
    )
}

/// Build one checked scalar transaction without using interval arithmetic in expected values.
fn run<P: Scalar + 'static>(
    function: Elementwise,
    center: P,
    radius: P,
) -> TestResult<(f64, f64, f64)> {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[center],
        &[radius],
    )?;
    let result = execute(
        &mut heap,
        OperationDraft::map_elementwise(contract.clone(), contract, function)?,
        vec![source.value()],
    )?;
    entry(&heap, output(&result)?, 0)
}
