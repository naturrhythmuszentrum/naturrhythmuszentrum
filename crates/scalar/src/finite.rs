// SPDX-License-Identifier: AGPL-3.0-only
//! Finite primitive values with canonical zero.

use crate::{ArithmeticFailure, Scalar};

/// A finite primitive value, with every signed zero canonicalized to positive zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct Finite<P: Scalar> {
    /// The checked, canonical primitive.
    value: P,
}

impl<P: Scalar> Finite<P> {
    /// Exact positive zero.
    pub const ZERO: Self = Self { value: P::ZERO };
    /// Exact one.
    pub const ONE: Self = Self { value: P::ONE };

    /// Admit a finite primitive and canonicalize signed zero.
    ///
    /// # Errors
    /// Returns `ArithmeticFailure::NonFinite` for infinities and NaNs.
    pub fn new(value: P) -> Result<Self, ArithmeticFailure> {
        if !value.is_finite() {
            return Err(ArithmeticFailure::NonFinite);
        }
        Ok(Self {
            value: if value == P::ZERO { P::ZERO } else { value },
        })
    }

    /// The represented primitive value.
    #[must_use]
    pub const fn get(self) -> P {
        self.value
    }

    /// Exact sign reversal with canonical zero.
    #[must_use]
    pub fn negated(self) -> Self {
        Self {
            value: if self.value == P::ZERO {
                P::ZERO
            } else {
                -self.value
            },
        }
    }

    /// Exact absolute value.
    #[must_use]
    pub fn abs(self) -> Self {
        Self {
            value: self.value.abs(),
        }
    }

    /// The adjacent finite value toward positive infinity.
    ///
    /// # Errors
    /// Returns `ArithmeticFailure::Overflow` at the positive finite limit.
    pub fn next_up(self) -> Result<Self, ArithmeticFailure> {
        Self::new(self.value.next_up()).map_err(|_| ArithmeticFailure::Overflow)
    }

    /// The adjacent finite value toward negative infinity.
    ///
    /// # Errors
    /// Returns `ArithmeticFailure::Overflow` at the negative finite limit.
    pub fn next_down(self) -> Result<Self, ArithmeticFailure> {
        Self::new(self.value.next_down()).map_err(|_| ArithmeticFailure::Overflow)
    }
}

impl Finite<f32> {
    /// Widen a represented binary32 value exactly into binary64.
    #[must_use]
    pub fn widen(self) -> Finite<f64> {
        Finite {
            value: f64::from(self.value),
        }
    }
}
