// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded floating decoding of exact numeric spellings, without symbolic arithmetic.

mod evaluate;
mod limits;
mod syntax;

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};
pub use limits::LiteralLimits;
use limits::Work;
use syntax::Decimal;

/// A checked, borrowed integer, decimal, scientific literal or single fraction.
///
/// Each fraction side is one signed decimal/scientific literal. Whitespace,
/// separators, nonfinite spellings and expressions are rejected. Digit limits
/// count all significand digits, including leading zeros, across both sides;
/// exponent limits bound both written and effective decimal exponents.
#[derive(Debug, Clone, Copy)]
pub struct NumericLiteral<'a> {
    /// Original bounded spelling, used only to select a floating candidate.
    text: &'a str,
    /// First literal, or the numerator of a fraction.
    numerator: Decimal<'a>,
    /// Optional literal denominator.
    denominator: Option<Decimal<'a>>,
    /// Prepaid text scanning and remaining arithmetic work.
    work: Work,
}

/// A finite candidate, exact-literal enclosure and outward candidate displacement.
#[derive(Debug, Clone, Copy)]
pub struct DecodedLiteral<P: Scalar> {
    /// Finite represented candidate; it need not be correctly rounded for fractions.
    center: Finite<P>,
    /// Enclosure of the mathematical numeric spelling.
    enclosure: Enclosure<P>,
    /// Numerical import error, with no physical or statistical interpretation.
    displacement: Finite<P>,
    /// Work units reserved for scans and enclosure evaluation.
    work_used: u64,
}

impl<'a> NumericLiteral<'a> {
    /// Validate grammar and limits before floating evaluation.
    ///
    /// # Errors
    /// Refuses malformed text or digit, exponent or scanning-work exhaustion.
    pub fn new(text: &'a str, limits: LiteralLimits) -> Result<Self, ArithmeticFailure> {
        let work = Work::new(text.len(), limits.max_work())?;
        let (numerator, denominator) = if let Some((top, bottom)) = text.split_once('/') {
            (
                syntax::decimal(top, limits)?,
                Some(syntax::decimal(bottom, limits)?),
            )
        } else {
            (syntax::decimal(text, limits)?, None)
        };
        let digits = numerator
            .digits()
            .checked_add(denominator.map_or(0, Decimal::digits))
            .ok_or(ArithmeticFailure::ResourceLimit)?;
        if digits > limits.max_digits() {
            return Err(ArithmeticFailure::ResourceLimit);
        }
        Ok(Self {
            text,
            numerator,
            denominator,
            work,
        })
    }

    /// Enclose the exact spelling and bound the selected finite candidate's error.
    ///
    /// Digits use enclosed multiply-by-ten/add; the decimal exponent uses bounded
    /// repeated multiplication or division by ten. A native parser selects the
    /// candidate for a single literal; a fraction uses its lower endpoint. No
    /// correctness claim relies on the candidate's rounding. Intermediates may
    /// refuse even when a more sophisticated conversion could represent the value.
    ///
    /// # Errors
    /// Refuses work exhaustion, overflowing intermediates, an enclosure containing
    /// zero in the denominator, or a nonfinite candidate/displacement.
    pub fn decode<P: Scalar>(
        &self,
        context: &ArithmeticContext,
    ) -> Result<DecodedLiteral<P>, ArithmeticFailure> {
        self.evaluate(context).map_err(|error| match error {
            ArithmeticFailure::Overflow | ArithmeticFailure::NonFinite => {
                ArithmeticFailure::LiteralRange
            }
            other => other,
        })
    }

    /// Perform admitted floating evaluation with a fresh copy of the bounded work state.
    fn evaluate<P: Scalar>(
        &self,
        context: &ArithmeticContext,
    ) -> Result<DecodedLiteral<P>, ArithmeticFailure> {
        let mut work = self.work;
        let numerator = evaluate::decimal(&self.numerator, context, &mut work)?;
        let enclosure = if let Some(denominator) = self.denominator {
            let divisor = evaluate::decimal(&denominator, context, &mut work)?;
            work.charge(1)?;
            numerator.div(divisor, context)?
        } else {
            numerator
        };
        let center = if self.denominator.is_none() {
            Finite::new(P::parse_nearest(self.text).ok_or(ArithmeticFailure::InvalidLiteral)?)?
        } else {
            enclosure.lower()
        };
        work.charge(1)?;
        let displacement = enclosure.displacement_from(center, context)?;
        Ok(DecodedLiteral {
            center,
            enclosure,
            displacement,
            work_used: work.used(),
        })
    }
}

impl<P: Scalar> DecodedLiteral<P> {
    /// The selected finite candidate.
    #[must_use]
    pub const fn center(self) -> Finite<P> {
        self.center
    }
    /// The enclosure of the exact numeric literal.
    #[must_use]
    pub const fn enclosure(self) -> Enclosure<P> {
        self.enclosure
    }
    /// Outward upper bound on the candidate's distance from the exact literal.
    #[must_use]
    pub const fn displacement(self) -> Finite<P> {
        self.displacement
    }
    /// Charged work, including prepaid grammar/candidate scans.
    #[must_use]
    pub const fn work_used(self) -> u64 {
        self.work_used
    }
}
