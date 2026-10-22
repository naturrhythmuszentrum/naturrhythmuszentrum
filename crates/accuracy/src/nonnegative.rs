// SPDX-License-Identifier: AGPL-3.0-only
//! Finite nonnegative magnitudes and outward composition.
//!
//! # Magnitude composition
//!
//! `Nonnegative<P>::new` admits finite nonnegative magnitudes and canonicalizes signed zero. Its private state proves that numerical invariant, not that a caller-supplied magnitude bounds a physical error. `add` returns an outward upper sum and `scale` an outward upper product, including underflow. Each delegates to scalar enclosures under an explicit `ArithmeticContext`; an unavailable finite endpoint returns the scalar error through `AccuracyFailure`. No fallible arithmetic is hidden behind operator traits.

use crate::AccuracyFailure;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// A finite nonnegative represented magnitude, with canonical positive zero.
///
/// Admission proves only the numerical invariant. The caller establishes what
/// this magnitude bounds and retains its physical target and coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct Nonnegative<P: Scalar> {
    /// Checked finite magnitude.
    value: Finite<P>,
}

impl<P: Scalar> Nonnegative<P> {
    /// Exact zero bound.
    pub const ZERO: Self = Self {
        value: Finite::ZERO,
    };
    /// Exact unit magnitude.
    pub const ONE: Self = Self { value: Finite::ONE };

    /// Admit a finite nonnegative value and canonicalize signed zero.
    ///
    /// # Errors
    /// Returns `AccuracyFailure::InvalidBound` for negative or nonfinite values.
    pub fn new(value: P) -> Result<Self, AccuracyFailure> {
        let value = Finite::new(value).map_err(|_| AccuracyFailure::InvalidBound)?;
        if value < Finite::ZERO {
            return Err(AccuracyFailure::InvalidBound);
        }
        Ok(Self { value })
    }

    /// The represented bound magnitude.
    #[must_use]
    pub const fn get(self) -> P {
        self.value.get()
    }

    /// The checked finite magnitude for scalar enclosure operations.
    #[must_use]
    pub const fn finite(self) -> Finite<P> {
        self.value
    }

    /// Bound the exact real sum of two magnitudes from above.
    ///
    /// # Errors
    /// Returns the scalar cause if a finite outward result is unavailable.
    pub fn add(self, rhs: Self, context: &ArithmeticContext) -> Result<Self, AccuracyFailure> {
        let result =
            Enclosure::singleton(self.value).add(Enclosure::singleton(rhs.value), context)?;
        Self::new(result.upper().get())
    }

    /// Bound a product, including scaling by a nonnegative coefficient bound.
    ///
    /// # Errors
    /// Returns the scalar cause if a finite outward result is unavailable.
    pub fn scale(self, factor: Self, context: &ArithmeticContext) -> Result<Self, AccuracyFailure> {
        let result =
            Enclosure::singleton(self.value).mul(Enclosure::singleton(factor.value), context)?;
        Self::new(result.upper().get())
    }
}
