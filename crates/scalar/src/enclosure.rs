// SPDX-License-Identifier: AGPL-3.0-only
//! Finite closed real sets used as temporary kernel verification objects.

use crate::{ArithmeticContext, ArithmeticFailure, Finite, Scalar, primitive};

/// A closed real enclosure with finite, ordered, canonical endpoints.
///
/// This type carries no physical quantity, source model or heap evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enclosure<P: Scalar> {
    /// Least included real value.
    lower: Finite<P>,
    /// Greatest included real value.
    upper: Finite<P>,
}

impl<P: Scalar> Enclosure<P> {
    /// Enclose an exactly represented finite value.
    #[must_use]
    pub const fn singleton(value: Finite<P>) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    /// Admit finite, ordered endpoints and canonicalize signed zeros.
    ///
    /// # Errors
    /// Refuses nonfinite or inverted endpoints.
    pub fn new(lower: P, upper: P) -> Result<Self, ArithmeticFailure> {
        let lower = Finite::new(lower)?;
        let upper = Finite::new(upper)?;
        if lower > upper {
            return Err(ArithmeticFailure::InvertedEndpoints);
        }
        Ok(Self { lower, upper })
    }

    /// Lower endpoint.
    #[must_use]
    pub const fn lower(self) -> Finite<P> {
        self.lower
    }
    /// Upper endpoint.
    #[must_use]
    pub const fn upper(self) -> Finite<P> {
        self.upper
    }

    /// Whether a finite represented value belongs to this closed set.
    #[must_use]
    pub fn contains(self, value: Finite<P>) -> bool {
        self.lower <= value && value <= self.upper
    }

    /// Exact interval negation.
    #[must_use]
    pub fn negated(self) -> Self {
        Self {
            lower: self.upper.negated(),
            upper: self.lower.negated(),
        }
    }

    /// Exact absolute-value image, including a possible zero crossing.
    #[must_use]
    pub fn abs(self) -> Self {
        if self.lower.get() >= P::ZERO {
            return self;
        }
        if self.upper.get() <= P::ZERO {
            return self.negated();
        }
        let magnitude = if self.lower.abs() > self.upper {
            self.lower.abs()
        } else {
            self.upper
        };
        Self {
            lower: Finite::ZERO,
            upper: magnitude,
        }
    }

    /// Enclose addition using monotone endpoint pairs.
    ///
    /// # Errors
    /// Refuses a nonfinite rounded result or required outward neighbour.
    pub fn add(self, rhs: Self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        Self::new(
            primitive::add(self.lower.get(), rhs.lower.get())?
                .lower()
                .get(),
            primitive::add(self.upper.get(), rhs.upper.get())?
                .upper()
                .get(),
        )
    }

    /// Enclose subtraction using opposite endpoint pairs.
    ///
    /// # Errors
    /// Refuses a nonfinite rounded result or required outward neighbour.
    pub fn sub(self, rhs: Self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        Self::new(
            primitive::sub(self.lower.get(), rhs.upper.get())?
                .lower()
                .get(),
            primitive::sub(self.upper.get(), rhs.lower.get())?
                .upper()
                .get(),
        )
    }

    /// Enclose the hull of all four endpoint products.
    ///
    /// # Errors
    /// Refuses any overflowing endpoint operation, even if a later operation cancels it.
    pub fn mul(self, rhs: Self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        self.pairs(rhs, primitive::mul)
    }

    /// Enclose the hull of all four endpoint quotients.
    ///
    /// # Errors
    /// Refuses a zero-containing divisor or any overflowing endpoint operation.
    pub fn div(self, rhs: Self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        if rhs.contains(Finite::ZERO) {
            return Err(ArithmeticFailure::ZeroInDivisor);
        }
        self.pairs(rhs, primitive::div)
    }

    /// Square an enclosure, using nonnegativity even at underflow and zero crossings.
    ///
    /// # Errors
    /// Refuses any overflowing endpoint square or outward neighbour.
    pub fn square(self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        let magnitude = self.abs();
        let lower = primitive::mul(magnitude.lower.get(), magnitude.lower.get())?.lower();
        let upper = primitive::mul(magnitude.upper.get(), magnitude.upper.get())?.upper();
        Ok(Self {
            lower: if lower < Finite::ZERO {
                Finite::ZERO
            } else {
                lower
            },
            upper,
        })
    }

    /// Enclose square root by monotonicity on a nonnegative enclosure.
    ///
    /// # Errors
    /// Refuses a negative lower endpoint or any nonfinite outward endpoint.
    pub fn sqrt(self, _context: &ArithmeticContext) -> Result<Self, ArithmeticFailure> {
        if self.lower < Finite::ZERO {
            return Err(ArithmeticFailure::NegativeRoot);
        }
        Ok(Self {
            lower: primitive::sqrt(self.lower.get())?.lower(),
            upper: primitive::sqrt(self.upper.get())?.upper(),
        })
    }

    /// Evaluate a binary endpoint operation and take its enclosing hull.
    fn pairs(
        self,
        rhs: Self,
        operation: impl Fn(P, P) -> Result<Self, ArithmeticFailure>,
    ) -> Result<Self, ArithmeticFailure> {
        let mut result = operation(self.lower.get(), rhs.lower.get())?;
        for (left, right) in [
            (self.lower, rhs.upper),
            (self.upper, rhs.lower),
            (self.upper, rhs.upper),
        ] {
            let value = operation(left.get(), right.get())?;
            if value.lower < result.lower {
                result.lower = value.lower;
            }
            if value.upper > result.upper {
                result.upper = value.upper;
            }
        }
        Ok(result)
    }
}
