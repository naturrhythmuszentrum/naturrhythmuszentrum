// SPDX-License-Identifier: AGPL-3.0-only
//! Shared numerical recipe for a recognized exact rational factor of pi.

use super::{ConversionError, PiConstant};
use crate::coordinate_numeric::ConstantCoefficient;
use nrz_bindings::PiFactor;
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, Scalar};

/// Literal import and the admitted pi operation retain one complete numerical discrepancy.
pub(crate) fn decode<P: Scalar + 'static>(
    factor: (&str, PiFactor, u64),
    pi: &PiConstant<P>,
    literals: LiteralLimits,
    context: &ArithmeticContext,
    available: u64,
) -> Result<ConstantCoefficient<P>, ConversionError> {
    let (spelling, action, work) = factor;
    let remaining = available
        .checked_sub(work)
        .ok_or(ConversionError::ResourceLimit)?;
    let limits = LiteralLimits::new(
        literals.max_digits(),
        literals.max_exponent(),
        literals.max_work().min(remaining),
    );
    let decoded = NumericLiteral::new(spelling, limits)?.decode::<P>(context)?;
    let work = work
        .checked_add(decoded.work_used())
        .ok_or(ConversionError::ResourceLimit)?;
    if action == PiFactor::Rational {
        return ConstantCoefficient::enclosed(
            decoded.enclosure(),
            work.checked_add(1)
                .filter(|work| *work <= available)
                .ok_or(ConversionError::ResourceLimit)?,
            context,
        );
    }
    let work = work
        .checked_add(2)
        .filter(|work| *work <= available)
        .ok_or(ConversionError::ResourceLimit)?;
    let factor = match action {
        PiFactor::Multiply => pi.pi().enclosure(),
        PiFactor::Divide => pi.reciprocal(),
        PiFactor::Rational => return Err(ConversionError::Contract),
    };
    ConstantCoefficient::enclosed(decoded.enclosure().mul(factor, context)?, work, context)
}
