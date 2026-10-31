// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded floating decoding of exact numeric spellings, without symbolic arithmetic.
//!
//! # Exact spelling import contract
//!
//! `NumericLiteral::new` checks a borrowed signed integer, decimal or scientific spelling, or one fraction whose sides are such literals. `LiteralLimits` bounds combined significand digits, written and effective decimal exponents, and work. Eight work units per input byte prepay grammar and candidate scans; each enclosure operation costs one additional unit. Defaults permit 1024 digits, exponent magnitude 4096 and 100000 work units. Zero limits are valid refusing policies. Whitespace, expressions, separators and nonfinite spellings are rejected.
//!
//! Integer fractions first use exact bounded reduction to recognize representable normal dyadics; these return a singleton and zero displacement. Unsupported sizes, subnormals and nonrepresentable fractions retain the conservative path.
//!
//! Conservative decoding accumulates digits by enclosed multiply-by-ten/add and applies decimal scale by bounded repeated multiplication or division by ten. This avoids building an overflowing power-of-ten denominator for subnormal literals. A native parser selects a finite candidate for a single literal; a fraction uses its lower endpoint. `DecodedLiteral` exposes the candidate, the exact-spelling enclosure, its numerical displacement bound and charged work. It promises containment rather than tight or correctly rounded fraction candidates. Overflowing intermediates and zero-containing denominator enclosures refuse even when another conversion algorithm could succeed. Production decoding has no rational or CAS dependency.

mod evaluate;
mod exact;
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
        let mut used = 0;
        self.decode_with_work(context, &mut used)
    }

    /// Decode under the same literal ceiling and report consumed Work even when evaluation refuses.
    ///
    /// The reported count includes the scanning reservation from successful literal construction.
    /// # Errors
    /// Preserves every refusal of `decode` and reports this attempt's consumed Work on error.
    pub fn decode_with_work<P: Scalar>(
        &self,
        context: &ArithmeticContext,
        work_used: &mut u64,
    ) -> Result<DecodedLiteral<P>, ArithmeticFailure> {
        let mut work = self.work;
        let result = self.evaluate(context, &mut work);
        *work_used = work.used();
        result.map_err(|error| match error {
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
        work: &mut Work,
    ) -> Result<DecodedLiteral<P>, ArithmeticFailure> {
        if let Some(denominator) = self.denominator
            && let Some(decoded) = exact::fraction(&self.numerator, &denominator, work)?
        {
            return Ok(decoded);
        }
        let numerator = evaluate::decimal(&self.numerator, context, work)?;
        let enclosure = if let Some(denominator) = self.denominator {
            let divisor = evaluate::decimal(&denominator, context, work)?;
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
