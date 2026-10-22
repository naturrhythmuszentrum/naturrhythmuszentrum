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
    /// The sealed format contract requires its own exact representation tag, fixed by (´rule:scalar:format-identities´).
    const PRECISION: Precision;
    /// Additive identity, with positive zero bits.
    /// The scalar contract requires the canonical additive identity, fixed by (´rule:scalar:finite-identities´).
    const ZERO: Self;
    /// Multiplicative identity.
    /// The scalar contract requires the exact multiplicative identity, fixed by (´rule:scalar:finite-identities´).
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
    /// Explicit fused multiply-add with one rounding.
    #[must_use]
    fn mul_add(self, multiplier: Self, addend: Self) -> Self;
    /// Positive normal power-of-two scale; subnormals use the smallest normal.
    #[must_use]
    fn power_of_two_scale(self) -> Self;
    /// Square root rounded to nearest under the admitted environment.
    #[must_use]
    fn sqrt(self) -> Self;
    /// Smallest positive normal value.
    fn min_positive_normal() -> Self;
    /// Largest finite value.
    fn max_finite() -> Self;
    /// Exact conversion of a byte-sized unsigned integer.
    fn from_u8(value: u8) -> Self;
    /// Native numeric parser used only to select a candidate inside bounded decoding.
    fn parse_nearest(text: &str) -> Option<Self>;
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

/// Implement the primitive methods while keeping each format constant inspectable.
macro_rules! scalar_methods {
    ($mask:expr) => {
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
        fn mul_add(self, multiplier: Self, addend: Self) -> Self {
            self.mul_add(multiplier, addend)
        }
        fn power_of_two_scale(self) -> Self {
            let exponent = self.abs().to_bits() & $mask;
            if exponent == 0 {
                Self::MIN_POSITIVE
            } else {
                Self::from_bits(exponent)
            }
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
        fn from_u8(value: u8) -> Self {
            Self::from(value)
        }
        fn parse_nearest(text: &str) -> Option<Self> {
            text.parse().ok()
        }
        fn to_f64(self) -> f64 {
            f64::from(self)
        }
        fn same_bits(self, other: Self) -> bool {
            self.to_bits() == other.to_bits()
        }
    };
}

impl Scalar for f32 {
    /// This tag identifies the sealed primitive format under (´rule:scalar:format-identities´).
    /// ´const:scalar:f32-precision´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f32-precision-form-x809900e6´
    const PRECISION: Precision = Precision::F32;
    /// Canonical positive zero supplies the additive identity under (´rule:scalar:finite-identities´).
    /// ´const:scalar:f32-zero´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f32-zero-form-x1d0a2219´
    const ZERO: Self = 0.0;
    /// Exact unity supplies the multiplicative identity under (´rule:scalar:finite-identities´).
    /// ´const:scalar:f32-one´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f32-one-form-x182e1cd2´
    const ONE: Self = 1.0;

    scalar_methods!(0x7f80_0000);
}

impl Scalar for f64 {
    /// This tag identifies the sealed primitive format under (´rule:scalar:format-identities´).
    /// ´const:scalar:f64-precision´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f64-precision-form-x80a79ce1´
    const PRECISION: Precision = Precision::F64;
    /// Canonical positive zero supplies the additive identity under (´rule:scalar:finite-identities´).
    /// ´const:scalar:f64-zero´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f64-zero-form-x1d0a2219´
    const ZERO: Self = 0.0;
    /// Exact unity supplies the multiplicative identity under (´rule:scalar:finite-identities´).
    /// ´const:scalar:f64-one´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:scalar:f64-one-form-x182e1cd2´
    const ONE: Self = 1.0;

    scalar_methods!(0x7ff0_0000_0000_0000);
}
