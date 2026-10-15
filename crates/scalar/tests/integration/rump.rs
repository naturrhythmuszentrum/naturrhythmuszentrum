// SPDX-License-Identifier: AGPL-3.0-only
//! Rump's polynomial evaluated through the public enclosure facade.

use crate::testing::{TestResult, contains, point};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Scalar};
use num_bigint::BigInt;
use num_rational::Ratio;

/// Small natural power evaluated in a fixed, explicit multiplication order.
fn power<P: Scalar>(
    base: Enclosure<P>,
    exponent: usize,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let mut product = point(P::ONE)?;
    for _ in 0..exponent {
        product = product.mul(base, context)?;
    }
    Ok(product)
}

/// The documented polynomial order, with exactly representable coefficients.
fn evaluate<P: Scalar>(
    values: [P; 7],
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let [
        first_coordinate,
        second_coordinate,
        sextic_factor,
        mixed_factor,
        quartic_factor,
        divisor,
        octic_factor,
    ] = values;
    let first_input = point(first_coordinate)?;
    let second_input = point(second_coordinate)?;
    let first_square = power(first_input, 2, context)?;
    let second_square = power(second_input, 2, context)?;
    let quartic = power(second_input, 4, context)?;
    let sextic = power(second_input, 6, context)?;
    let octic = power(second_input, 8, context)?;
    let first = point(sextic_factor)?.mul(sextic, context)?;
    let inside = point(mixed_factor)?
        .mul(first_square, context)?
        .mul(second_square, context)?
        .sub(sextic, context)?
        .sub(point(quartic_factor)?.mul(quartic, context)?, context)?
        .sub(point(divisor)?, context)?;
    let middle = first_square.mul(inside, context)?;
    let last = point(octic_factor)?.mul(octic, context)?;
    let quotient = first_input.div(point(divisor)?.mul(second_input, context)?, context)?;
    first
        .add(middle, context)?
        .add(last, context)?
        .add(quotient, context)
}

/// The expected target is the independent exact fraction, never an enclosure calculation.
/// ´test:integration:rump-polynomial-enclosure´
#[test]
fn rump_polynomial_enclosure() -> TestResult {
    let context = ArithmeticContext::new()?;
    let exact = Ratio::new(BigInt::from(-54767), BigInt::from(66192));
    contains(
        evaluate(
            [77617.0_f64, 33096.0, 333.75, 11.0, 121.0, 2.0, 5.5],
            &context,
        )?,
        &exact,
    );
    contains(
        evaluate(
            [77617.0_f32, 33096.0, 333.75, 11.0, 121.0, 2.0, 5.5],
            &context,
        )?,
        &exact,
    );
    Ok(())
}
