// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed primitive formats; raw operations do not themselves certify a result.

use core::fmt::Debug;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// The two supported IEEE binary formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Binary32, with 24 significant binary digits.
    F32,
    /// Binary64, with 53 significant binary digits.
    F64,
}

/// Primitive operations for the sealed `f32` and `f64` formats.
///
/// Plain buffers remain primitive floats. Use `Finite` and `Enclosure` for
/// checked admission and certified arithmetic under an `ArithmeticContext`.
pub trait Scalar:
    private::Sealed
    + Copy
    + Debug
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// Format identity.
    const PRECISION: Precision;
    /// Additive identity, with positive zero bits.
    const ZERO: Self;
    /// Multiplicative identity.
    const ONE: Self;
    /// Whether the value is finite.
    fn is_finite(self) -> bool;
    /// Exact magnitude.
    #[must_use]
    fn abs(self) -> Self;
    /// Next representable value toward positive infinity, possibly infinite.
    #[must_use]
    fn next_up(self) -> Self;
    /// Next representable value toward negative infinity, possibly infinite.
    #[must_use]
    fn next_down(self) -> Self;
    /// Square root rounded to nearest under the admitted environment.
    #[must_use]
    fn sqrt(self) -> Self;
    /// Smallest positive normal value.
    fn min_positive_normal() -> Self;
    /// Largest finite value.
    fn max_finite() -> Self;
    /// Exact representation in binary64.
    fn to_f64(self) -> f64;
    /// Whether two primitive values have identical bit patterns.
    fn same_bits(self, other: Self) -> bool;
}

/// Only the two built-in binary formats can implement the scalar contract.
mod private {
    /// An inaccessible supertrait prevents external implementations.
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// Implement only the primitive format operations, without exactness guesses.
macro_rules! impl_scalar {
    ($ty:ty, $precision:ident) => {
        impl Scalar for $ty {
            const PRECISION: Precision = Precision::$precision;
            const ZERO: Self = 0.0;
            const ONE: Self = 1.0;
            fn is_finite(self) -> bool {
                self.is_finite()
            }
            fn abs(self) -> Self {
                self.abs()
            }
            fn next_up(self) -> Self {
                self.next_up()
            }
            fn next_down(self) -> Self {
                self.next_down()
            }
            fn sqrt(self) -> Self {
                self.sqrt()
            }
            fn min_positive_normal() -> Self {
                Self::MIN_POSITIVE
            }
            fn max_finite() -> Self {
                Self::MAX
            }
            fn to_f64(self) -> f64 {
                f64::from(self)
            }
            fn same_bits(self, other: Self) -> bool {
                self.to_bits() == other.to_bits()
            }
        }
    };
}

impl_scalar!(f32, F32);
impl_scalar!(f64, F64);
