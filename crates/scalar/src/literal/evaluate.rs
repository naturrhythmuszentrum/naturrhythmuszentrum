// SPDX-License-Identifier: AGPL-3.0-only
//! Inclusion-preserving digit and decimal-scale evaluation.

use super::{limits::Work, syntax::Decimal};
use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};

/// Accumulate exact digits and then apply each bounded decimal-scale factor.
pub(crate) fn decimal<P: Scalar>(
    literal: &Decimal<'_>,
    context: &ArithmeticContext,
    work: &mut Work,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let ten = Enclosure::singleton(Finite::new(P::from_u8(10))?);
    let mut value = Enclosure::singleton(Finite::ZERO);
    for digit in literal.whole.bytes().chain(literal.fraction.bytes()) {
        work.charge(2)?;
        let unit = Enclosure::singleton(Finite::new(P::from_u8(digit - b'0'))?);
        value = value.mul(ten, context)?.add(unit, context)?;
    }
    for _ in 0..literal.exponent.unsigned_abs() {
        work.charge(1)?;
        value = if literal.exponent < 0 {
            nonnegative(value.div(ten, context)?)?
        } else {
            value.mul(ten, context)?
        };
    }
    Ok(if literal.negative {
        value.negated()
    } else {
        value
    })
}

/// A decimal magnitude is mathematically nonnegative, including after underflow.
fn nonnegative<P: Scalar>(value: Enclosure<P>) -> Result<Enclosure<P>, ArithmeticFailure> {
    let lower = if value.lower() < Finite::ZERO {
        P::ZERO
    } else {
        value.lower().get()
    };
    Enclosure::new(lower, value.upper().get())
}
