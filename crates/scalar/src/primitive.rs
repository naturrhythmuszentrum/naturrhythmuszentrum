// SPDX-License-Identifier: AGPL-3.0-only
//! Outward neighbours of correctly rounded primitives and proved identities.

use crate::{ArithmeticFailure, Enclosure, Finite, Scalar};

/// Enclose a correctly rounded result, including zero and subnormal results.
pub(crate) fn rounded<P: Scalar>(nearest: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    let value = Finite::new(nearest).map_err(|_| ArithmeticFailure::Overflow)?;
    Enclosure::new(value.next_down()?.get(), value.next_up()?.get())
}

/// Preserve only the exact zero-addition identities.
pub(crate) fn add<P: Scalar>(left: P, right: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    if left == P::ZERO || right == P::ZERO {
        return singleton(if left == P::ZERO { right } else { left });
    }
    rounded(left + right)
}

/// Identical finite operands have exactly zero difference.
pub(crate) fn sub<P: Scalar>(left: P, right: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    if left == right {
        return Ok(Enclosure::singleton(Finite::ZERO));
    }
    rounded(left - right)
}

/// Zero and unit multipliers are the only product exactness shortcuts.
pub(crate) fn mul<P: Scalar>(left: P, right: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    if left == P::ZERO || right == P::ZERO {
        return Ok(Enclosure::singleton(Finite::ZERO));
    }
    if left.abs() == P::ONE || right.abs() == P::ONE {
        return singleton(left * right);
    }
    rounded(left * right)
}

/// Zero numerator and unit divisors are exact after zero-divisor rejection.
pub(crate) fn div<P: Scalar>(left: P, right: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    if right == P::ZERO {
        return Err(ArithmeticFailure::ZeroInDivisor);
    }
    if left == P::ZERO || right.abs() == P::ONE {
        return singleton(left / right);
    }
    rounded(left / right)
}

/// Only the roots of zero and one bypass outward rounding.
pub(crate) fn sqrt<P: Scalar>(value: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    if value < P::ZERO {
        return Err(ArithmeticFailure::NegativeRoot);
    }
    if value == P::ZERO || value == P::ONE {
        return singleton(value);
    }
    rounded(value.sqrt())
}

/// Turn an established exact finite result into a singleton.
fn singleton<P: Scalar>(value: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}
