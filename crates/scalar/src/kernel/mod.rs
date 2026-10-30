// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed kernel laws with exact dyadic endpoint certificates and explicit Work.
//!
//! Each record is a conditional mathematical enclosure over its complete operand
//! boxes. It proves no source, heap version, context, physical coverage or exact
//! target correspondence. The receiving owner must prove those premises separately.
//! The law fixes all rational endpoint relations; no supplied success flag is trusted.

mod budget;
mod dyadic;

use crate::{
    ArithmeticContext, ArithmeticFailure, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};
pub use budget::{KernelBudget, KernelUsage};
pub use dyadic::{Dyadic, DyadicInterval};

/// A failed certificate attempt, with all earlier Work charges retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KernelError {
    /// Explicit cumulative certificate Work is exhausted.
    #[error("kernel certificate work limit exceeded")]
    WorkLimit,
    /// A rational constant must have a positive denominator.
    #[error("kernel rational denominator is zero")]
    ZeroDenominator,
    /// This certificate family admits square root only on strictly positive boxes.
    #[error("kernel square root requires a positive lower endpoint")]
    PositiveRoot,
    /// The underlying outward arithmetic or bounded literal import refused.
    #[error("kernel certificate arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Closed mathematical operations and their complete exact operand coverage.
///
/// For output `[l,u]`, rational import checks `l <= n/d <= u`; sum checks
/// `l <= a.lower+b.lower` and `a.upper+b.upper <= u`; product checks both
/// inequalities for every endpoint product. Reciprocal first excludes zero,
/// then checks `l <= 1/a.upper` and `1/a.lower <= u`. Positive square root
/// checks `0 < a.lower`, `0 <= l <= u`, `l*l <= a.lower` and `a.upper <= u*u`.
/// All inputs and the output must be ordered. These are exact rational relations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelLaw {
    /// Exact rational constant, independently checkable without its decoding procedure.
    Rational {
        /// Exact signed numerator.
        numerator: i64,
        /// Exact positive denominator.
        denominator: u64,
    },
    /// Exact real addition over the entire Cartesian product of the operand boxes.
    Sum {
        /// Complete covered left operand set.
        left: DyadicInterval,
        /// Complete covered right operand set.
        right: DyadicInterval,
    },
    /// Exact real multiplication over the entire Cartesian product of the operand boxes.
    Product {
        /// Complete covered left operand set.
        left: DyadicInterval,
        /// Complete covered right operand set.
        right: DyadicInterval,
    },
    /// Exact reciprocal on one connected interval excluding zero.
    Reciprocal {
        /// Complete covered operand set; both endpoints have the same strict sign.
        input: DyadicInterval,
    },
    /// Nonnegative square root on a strictly positive operand box.
    SquareRoot {
        /// Complete covered positive operand set.
        input: DyadicInterval,
    },
}

/// Pure mathematical candidate for independent exact replay; contains no floats.
///
/// Public fields deliberately confer no authority. A consumer reconstructs every
/// relation specified by `KernelLaw` and binds every input to its original scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelCertificate {
    /// Closed law and complete dyadic operand sets.
    pub law: KernelLaw,
    /// Claimed outward result; both endpoints require independent exact checking.
    pub output: DyadicInterval,
}

/// A numerical evaluation with a separately exportable mathematical certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelEvaluation<P: Scalar> {
    /// Finite numerical endpoints returned by the existing outward operations.
    enclosure: Enclosure<P>,
    /// Integer-only mathematical record, retained independently of numerical storage.
    certificate: KernelCertificate,
}

impl<P: Scalar> KernelEvaluation<P> {
    /// Import one bounded exact rational constant and retain its original definition.
    ///
    /// Prepay 64 formatting units and the explicit literal attempt allowance before
    /// allocating the bounded spelling; failure never refunds either charge.
    /// # Errors
    /// Refuses a zero denominator, decoding limits, nonfinite enclosure or Work.
    pub fn rational(
        numerator: i64,
        denominator: u64,
        literals: LiteralLimits,
        context: &ArithmeticContext,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(1)?;
        if denominator == 0 {
            return Err(KernelError::ZeroDenominator);
        }
        budget.charge(64)?;
        budget.charge(literals.max_work())?;
        let spelling = format!("{numerator}/{denominator}");
        let enclosure = NumericLiteral::new(&spelling, literals)?
            .decode::<P>(context)?
            .enclosure();
        Self::finish(
            enclosure,
            KernelLaw::Rational {
                numerator,
                denominator,
            },
            budget,
        )
    }

    /// Enclose exact addition over both complete input boxes and record their dyadics.
    /// # Errors
    /// Refuses the underlying addition or cumulative Work.
    pub fn sum(
        left: Enclosure<P>,
        right: Enclosure<P>,
        context: &ArithmeticContext,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(2)?;
        let enclosure = left.add(right, context)?;
        budget.charge(32)?;
        let law = KernelLaw::Sum {
            left: DyadicInterval::from_enclosure(left),
            right: DyadicInterval::from_enclosure(right),
        };
        Self::finish(enclosure, law, budget)
    }

    /// Enclose exact multiplication over both complete input boxes.
    /// # Errors
    /// Refuses the underlying endpoint products or cumulative Work.
    pub fn product(
        left: Enclosure<P>,
        right: Enclosure<P>,
        context: &ArithmeticContext,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(4)?;
        let enclosure = left.mul(right, context)?;
        budget.charge(32)?;
        let law = KernelLaw::Product {
            left: DyadicInterval::from_enclosure(left),
            right: DyadicInterval::from_enclosure(right),
        };
        Self::finish(enclosure, law, budget)
    }

    /// Enclose the reciprocal of a box proved away from zero by its endpoints.
    /// # Errors
    /// Refuses a zero-containing divisor, unavailable finite bounds or Work.
    pub fn reciprocal(
        input: Enclosure<P>,
        context: &ArithmeticContext,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(4)?;
        let enclosure = Enclosure::singleton(Finite::ONE).div(input, context)?;
        budget.charge(16)?;
        Self::finish(
            enclosure,
            KernelLaw::Reciprocal {
                input: DyadicInterval::from_enclosure(input),
            },
            budget,
        )
    }

    /// Enclose a strictly positive square root with squared-endpoint proof obligations.
    /// # Errors
    /// Refuses a nonpositive lower endpoint, scalar arithmetic or cumulative Work.
    pub fn square_root(
        input: Enclosure<P>,
        context: &ArithmeticContext,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(1)?;
        if input.lower() <= Finite::ZERO {
            return Err(KernelError::PositiveRoot);
        }
        budget.charge(2)?;
        let enclosure = input.sqrt(context)?;
        budget.charge(16)?;
        Self::finish(
            enclosure,
            KernelLaw::SquareRoot {
                input: DyadicInterval::from_enclosure(input),
            },
            budget,
        )
    }

    /// Read the computed numerical interval; this getter performs no certificate export.
    #[must_use]
    pub const fn enclosure(self) -> Enclosure<P> {
        self.enclosure
    }

    /// Export one fixed-size pure record, charging its conservative logical copy size.
    /// # Errors
    /// Refuses cumulative Work without returning a partial certificate.
    pub fn export(&self, budget: &mut KernelBudget) -> Result<KernelCertificate, KernelError> {
        budget.charge(128)?;
        budget.exported();
        Ok(self.certificate)
    }

    /// Prepay two exact endpoint decodings and fixed record assembly before publication.
    fn finish(
        enclosure: Enclosure<P>,
        law: KernelLaw,
        budget: &mut KernelBudget,
    ) -> Result<Self, KernelError> {
        budget.charge(24)?;
        let certificate = KernelCertificate {
            law,
            output: DyadicInterval::from_enclosure(enclosure),
        };
        budget.constructed();
        Ok(Self {
            enclosure,
            certificate,
        })
    }
}
