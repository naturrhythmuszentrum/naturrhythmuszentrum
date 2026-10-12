// SPDX-License-Identifier: AGPL-3.0-only
//! Provisional validated scalar arithmetic for binary32 and binary64.
//! The interface remains provisional until the units and matrix bites settle it.
//! The rounding assumptions follow dec:vision:precision-selection and cav:vision:validated-arithmetic-assumptions.

use core::fmt::Debug;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// Operations required by the sealed binary scalar contract.
/// Only binary32 and binary64 are admitted (dec:vision:precision-selection).
///
/// TODO ´todo:code:keep-the-binary-scalar-contract-dimension´: keep the binary scalar contract dimension-neutral beneath quantity and discrete-count wrappers under dec:vision:storage-and-scalar-wrapping.
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
    /// Additive identity.
    const ZERO: Self;
    /// Multiplicative identity.
    const ONE: Self;
    /// Whether the value is finite.
    fn is_finite(self) -> bool;
    /// Magnitude of the value.
    #[must_use]
    fn abs(self) -> Self;
    /// Next representable value toward positive infinity.
    #[must_use]
    fn next_up(self) -> Self;
    /// Next representable value toward negative infinity.
    #[must_use]
    fn next_down(self) -> Self;
    /// Fused multiply-add, with one rounding.
    #[must_use]
    fn mul_add(self, multiplier: Self, addend: Self) -> Self;
    /// Square root rounded to nearest.
    #[must_use]
    fn sqrt(self) -> Self;
    /// Smallest positive normal value.
    fn min_positive_normal() -> Self;
    /// Largest finite value.
    fn max_finite() -> Self;
    /// Conservative threshold above which a nonzero FMA residual cannot underflow.
    fn residual_threshold() -> Self;
    /// Power of two no greater than a positive normal value, or the smallest normal for a subnormal.
    #[must_use]
    fn power_of_two_scale(self) -> Self;
    /// Exact widening into binary64.
    fn to_f64(self) -> f64;
    /// Whether two values have identical bit patterns.
    fn same_bits(self, other: Self) -> bool;
}

/// Sealing prevents other scalar implementations.
mod private {
    /// Only primitive binary formats implement this marker.
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// Implement the sealed primitive contract for the binary32 format.
macro_rules! impl_scalar {
    ($ty:ty, $bits:ty, $mantissa:expr, $exponent_mask:expr, $one_exponent:expr) => {
        impl Scalar for $ty {
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
            fn mul_add(self, multiplier: Self, addend: Self) -> Self {
                self.mul_add(multiplier, addend)
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
            fn residual_threshold() -> Self {
                Self::MIN_POSITIVE.sqrt()
            }
            fn power_of_two_scale(self) -> Self {
                let exponent = self.abs().to_bits() & $exponent_mask;
                if exponent == 0 {
                    Self::MIN_POSITIVE
                } else {
                    Self::from_bits(exponent)
                }
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

impl_scalar!(f32, u32, 23, 0x7f80_0000, 0x3f80_0000);

impl Scalar for f64 {
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
    fn mul_add(self, multiplier: Self, addend: Self) -> Self {
        self.mul_add(multiplier, addend)
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
    fn residual_threshold() -> Self {
        Self::MIN_POSITIVE.sqrt()
    }
    fn power_of_two_scale(self) -> Self {
        let exponent = self.abs().to_bits() & 0x7ff0_0000_0000_0000;
        if exponent == 0 {
            Self::MIN_POSITIVE
        } else {
            Self::from_bits(exponent)
        }
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn same_bits(self, other: Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

/// A refusal to claim a finite enclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArithmeticFailure {
    /// An input endpoint is not finite.
    #[error("a non-finite endpoint cannot be enclosed")]
    NonFiniteEndpoint,
    /// The proposed lower endpoint exceeds its upper endpoint.
    #[error("inverted interval endpoints cannot be enclosed")]
    InvertedEndpoints,
    /// Division by an interval containing zero is undefined.
    #[error("division by an interval containing zero cannot be enclosed")]
    DivisorContainsZero,
    /// Real square root is undefined over a negative lower endpoint.
    #[error("square root of a negative interval cannot be enclosed")]
    NegativeSquareRoot,
    /// A result needs an infinite endpoint.
    #[error("an overflowing result cannot be enclosed finitely")]
    Overflow,
    /// Reduction inputs have incompatible lengths.
    #[error("slices of unequal length cannot form a dot product")]
    LengthMismatch,
}

/// A closed interval of finite binary values.
///
/// TODO ´todo:code:wrap-intervals-with-value-dimensions-measures´: wrap intervals with value dimensions, measures and kinds and intrinsic measurement and computational companions under dec:vision:storage-and-scalar-wrapping and def:vision:intrinsic-uncertainty-companions.
#[derive(Debug, Clone, Copy)]
pub struct Interval<T: Scalar> {
    /// Lower endpoint.
    lo: T,
    /// Upper endpoint.
    hi: T,
}

/// Conservatively step both ends unless exactness is established.
///
/// TODO ´todo:code:retain-primitive-rounding-bounds-as-computational´: retain primitive rounding bounds as computational uncertainty with independently derived descriptors at the quantity boundary under rule:vision:uncertainty-unit-algebra.
fn rounded<T: Scalar>(nearest: T, exact: bool) -> Result<Interval<T>, ArithmeticFailure> {
    if !nearest.is_finite() {
        return Err(ArithmeticFailure::Overflow);
    }
    if exact {
        return Interval::point(nearest);
    }
    let lo = nearest.next_down();
    let hi = nearest.next_up();
    if !lo.is_finite() || !hi.is_finite() {
        return Err(ArithmeticFailure::Overflow);
    }
    Interval::new(lo, hi)
}

/// Exact two-sum residual under round-to-nearest, away from underflow.
fn two_sum_residual<T: Scalar>(a: T, b: T, sum: T) -> T {
    let back = sum - a;
    (a - (sum - back)) + (b - back)
}

/// Whether an arithmetic residual can safely establish exactness.
fn normal_enough<T: Scalar>(a: T, b: T, result: T) -> bool {
    let threshold = T::min_positive_normal() + T::min_positive_normal();
    (a.same_bits(T::ZERO) || a.abs() >= threshold)
        && (b.same_bits(T::ZERO) || b.abs() >= threshold)
        && result.abs() >= threshold
}

/// Enclose one addition.
fn add_value<T: Scalar>(a: T, b: T) -> Result<Interval<T>, ArithmeticFailure> {
    let result = a + b;
    let exact = a.same_bits(T::ZERO)
        || b.same_bits(T::ZERO)
        || a.same_bits(-b)
        || (result.is_finite()
            && normal_enough(a, b, result)
            && two_sum_residual(a, b, result).same_bits(T::ZERO));
    rounded(result, exact)
}

/// Enclose one product, using the FMA residual only when it cannot underflow.
fn mul_value<T: Scalar>(a: T, b: T) -> Result<Interval<T>, ArithmeticFailure> {
    let result = a * b;
    let exact = if a.same_bits(T::ZERO) || b.same_bits(T::ZERO) {
        true
    } else {
        result.is_finite()
            && result.abs() >= T::residual_threshold()
            && normal_enough(a, b, result)
            && a.mul_add(b, -result).same_bits(T::ZERO)
    };
    rounded(result, exact)
}

/// Enclose one quotient, using the FMA remainder only when it cannot underflow.
fn div_value<T: Scalar>(a: T, b: T) -> Result<Interval<T>, ArithmeticFailure> {
    let result = a / b;
    let exact = if a.same_bits(T::ZERO) {
        true
    } else {
        result.is_finite()
            && a.abs() >= T::residual_threshold()
            && normal_enough(a, b, result)
            && (-result).mul_add(b, a).same_bits(T::ZERO)
    };
    rounded(result, exact)
}

/// Enclose one square root, using the FMA remainder only when it cannot underflow.
fn sqrt_value<T: Scalar>(a: T) -> Result<Interval<T>, ArithmeticFailure> {
    let result = a.sqrt();
    let exact = if a.same_bits(T::ZERO) {
        true
    } else {
        result.is_finite()
            && a >= T::residual_threshold()
            && normal_enough(a, result, result)
            && (-result).mul_add(result, a).same_bits(T::ZERO)
    };
    rounded(result, exact)
}

impl<T: Scalar> Interval<T> {
    /// Make an exactly represented point.
    ///
    /// TODO ´todo:code:distinguish-singleton-enclosure-construction-from-affine´: distinguish singleton enclosure construction from affine physical points and exact discrete counts under rule:vision:semantic-kinds-and-roles and def:vision:counts.
    ///
    /// # Errors
    ///
    /// A non-finite input returns `ArithmeticFailure::NonFiniteEndpoint`.
    pub fn point(value: T) -> Result<Self, ArithmeticFailure> {
        if !value.is_finite() {
            return Err(ArithmeticFailure::NonFiniteEndpoint);
        }
        Ok(Self {
            lo: value,
            hi: value,
        })
    }

    /// Make a checked finite enclosure.
    ///
    /// TODO ´todo:code:classify-imported-enclosures-as-carried-input´: classify imported enclosures as carried input evidence with explicit value and uncertainty descriptors under def:vision:intrinsic-uncertainty-companions and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// Non-finite or inverted endpoints return an `ArithmeticFailure`.
    pub fn new(lo: T, hi: T) -> Result<Self, ArithmeticFailure> {
        if !lo.is_finite() || !hi.is_finite() {
            return Err(ArithmeticFailure::NonFiniteEndpoint);
        }
        if lo > hi {
            return Err(ArithmeticFailure::InvertedEndpoints);
        }
        Ok(Self { lo, hi })
    }

    /// Lower endpoint.
    ///
    /// TODO ´todo:code:preserve-the-lower-endpoint-quantity-descriptor´: preserve the lower endpoint quantity descriptor through typed access under dec:vision:storage-and-scalar-wrapping.
    pub const fn lo(self) -> T {
        self.lo
    }
    /// Upper endpoint.
    ///
    /// TODO ´todo:code:preserve-the-upper-endpoint-quantity-descriptor´: preserve the upper endpoint quantity descriptor through typed access under dec:vision:storage-and-scalar-wrapping.
    pub const fn hi(self) -> T {
        self.hi
    }
    /// Whether a point is enclosed.
    ///
    /// TODO ´todo:code:check-containment-only-after-compatible-dimensions´: check containment only after compatible dimensions, measures, kinds and affine roles are established under req:vision:algebraic-unit-checks.
    pub fn contains(self, value: T) -> bool {
        self.lo <= value && value <= self.hi
    }

    /// Interval addition.
    ///
    /// TODO ´todo:code:gate-quantity-addition-by-compatible-kinds´: gate quantity addition by compatible kinds and affine roles while propagating both uncertainty sources under rule:vision:measure-conversions and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// Overflow that needs an infinite endpoint returns `ArithmeticFailure::Overflow`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, ArithmeticFailure> {
        Self::new(
            add_value(self.lo, rhs.lo)?.lo,
            add_value(self.hi, rhs.hi)?.hi,
        )
    }

    /// Interval subtraction.
    ///
    /// TODO ´todo:code:derive-difference-roles-for-quantity-subtraction´: derive difference roles for quantity subtraction and its uncertainty companion under rule:vision:measure-conversions and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// Overflow that needs an infinite endpoint returns `ArithmeticFailure::Overflow`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, ArithmeticFailure> {
        Self::new(
            add_value(self.lo, -rhs.hi)?.lo,
            add_value(self.hi, -rhs.lo)?.hi,
        )
    }

    /// Interval multiplication.
    ///
    /// TODO ´todo:code:derive-product-dimensions-and-uncertainty-kinds´: derive product dimensions and uncertainty kinds at the quantity layer under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// Overflow that needs an infinite endpoint returns `ArithmeticFailure::Overflow`.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, ArithmeticFailure> {
        let mut lo = T::max_finite();
        let mut hi = -T::max_finite();
        for (a, b) in [
            (self.lo, rhs.lo),
            (self.lo, rhs.hi),
            (self.hi, rhs.lo),
            (self.hi, rhs.hi),
        ] {
            let value = mul_value(a, b)?;
            if value.lo < lo {
                lo = value.lo;
            }
            if value.hi > hi {
                hi = value.hi;
            }
        }
        Self::new(lo, hi)
    }

    /// Interval division, refusing a divisor that contains zero.
    ///
    /// TODO ´todo:code:derive-quotient-dimensions-and-explicit-count´: derive quotient dimensions and explicit count-to-ratio outcomes without erasing uncertainty kinds under rule:vision:semantic-kinds-and-roles and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// A zero-containing divisor or overflow returns an `ArithmeticFailure`.
    pub fn checked_div(self, rhs: Self) -> Result<Self, ArithmeticFailure> {
        if rhs.contains(T::ZERO) {
            return Err(ArithmeticFailure::DivisorContainsZero);
        }
        let mut lo = T::max_finite();
        let mut hi = -T::max_finite();
        for (a, b) in [
            (self.lo, rhs.lo),
            (self.lo, rhs.hi),
            (self.hi, rhs.lo),
            (self.hi, rhs.hi),
        ] {
            let value = div_value(a, b)?;
            if value.lo < lo {
                lo = value.lo;
            }
            if value.hi > hi {
                hi = value.hi;
            }
        }
        Self::new(lo, hi)
    }

    /// Real square root of a nonnegative interval.
    ///
    /// TODO ´todo:code:admit-quantity-square-roots-only-when´: admit quantity square roots only when result dimensions and uncertainty roles are supported under req:vision:powers-and-factor-dimensions and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// A negative lower endpoint or overflow returns an `ArithmeticFailure`.
    pub fn sqrt(self) -> Result<Self, ArithmeticFailure> {
        if self.lo < T::ZERO {
            return Err(ArithmeticFailure::NegativeSquareRoot);
        }
        Self::new(sqrt_value(self.lo)?.lo, sqrt_value(self.hi)?.hi)
    }

    /// Square with the zero crossing handled directly.
    ///
    /// TODO ´todo:code:check-squared-quantity-dimensions-and-keep´: check squared quantity dimensions and keep variance kinds distinct from squared values under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
    ///
    /// # Errors
    ///
    /// Overflow that needs an infinite endpoint returns `ArithmeticFailure::Overflow`.
    pub fn square(self) -> Result<Self, ArithmeticFailure> {
        let lower = if self.contains(T::ZERO) {
            T::ZERO
        } else if self.lo > T::ZERO {
            mul_value(self.lo, self.lo)?.lo
        } else {
            mul_value(self.hi, self.hi)?.lo
        };
        let left = mul_value(self.lo, self.lo)?.hi;
        let right = mul_value(self.hi, self.hi)?.hi;
        Self::new(lower, if left > right { left } else { right })
    }
}

/// Enclose the exact dot product of two equally sized plain slices.
///
/// TODO ´todo:code:check-typed-dot-products-for-equal´: check typed dot products for equal summand dimensions and propagate intrinsic companions around this numerical primitive under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
///
/// # Errors
///
/// A length mismatch, non-finite input or overflow returns an `ArithmeticFailure`.
pub fn dot<T: Scalar>(left: &[T], right: &[T]) -> Result<Interval<T>, ArithmeticFailure> {
    if left.len() != right.len() {
        return Err(ArithmeticFailure::LengthMismatch);
    }
    let mut total = Interval::point(T::ZERO)?;
    for (&a, &b) in left.iter().zip(right) {
        total = total.checked_add(Interval::point(a)?.checked_mul(Interval::point(b)?)?)?;
    }
    Ok(total)
}

/// Enclose the exact sum of squares of a plain slice.
///
/// TODO ´todo:code:derive-squared-dimensions-for-homogeneous-reductions´: derive squared dimensions for homogeneous reductions and their computational bounds under def:vision:dimensions and rule:vision:uncertainty-unit-algebra.
///
/// # Errors
///
/// A non-finite input or overflow returns an `ArithmeticFailure`.
pub fn sum_squares<T: Scalar>(values: &[T]) -> Result<Interval<T>, ArithmeticFailure> {
    let mut total = Interval::point(T::ZERO)?;
    for &value in values {
        total = total.checked_add(Interval::point(value)?.square()?)?;
    }
    Ok(total)
}

/// Bound a Frobenius norm by scaling with an exact power of two.
///
/// TODO ´todo:code:assign-norm-bounds-their-block-or´: assign norm bounds their block or normalized-coordinate dimension while keeping numeric scaling separate from measures under rule:vision:heterogeneous-assembly and rule:vision:uncertainty-unit-algebra.
///
/// # Errors
///
/// A non-finite input or norm overflow returns an `ArithmeticFailure`.
pub fn frobenius_upper<T: Scalar>(values: &[T]) -> Result<T, ArithmeticFailure> {
    let mut largest = T::ZERO;
    for &value in values {
        if !value.is_finite() {
            return Err(ArithmeticFailure::NonFiniteEndpoint);
        }
        if value.abs() > largest {
            largest = value.abs();
        }
    }
    if largest.same_bits(T::ZERO) {
        return Ok(T::ZERO);
    }
    let scale = largest.power_of_two_scale();
    let mut total = Interval::point(T::ZERO)?;
    for &value in values {
        let scaled = Interval::point(value)?.checked_div(Interval::point(scale)?)?;
        total = total.checked_add(scaled.square()?)?;
    }
    Ok(total.sqrt()?.checked_mul(Interval::point(scale)?)?.hi())
}

/// TODO ´todo:code:preserve-quantity-and-uncertainty-descriptors-during´: preserve quantity and uncertainty descriptors during precision widening independently of measure conversion under rule:vision:precision-conversions and rule:vision:uncertainty-unit-algebra.
impl From<Interval<f32>> for Interval<f64> {
    fn from(value: Interval<f32>) -> Self {
        Self {
            lo: f64::from(value.lo),
            hi: f64::from(value.hi),
        }
    }
}

/// TODO ´todo:code:carry-narrowing-error-as-computational-uncertainty´: carry narrowing error as computational uncertainty without changing the value or uncertainty kind under rule:vision:precision-conversions and rule:vision:uncertainty-unit-algebra.
impl TryFrom<Interval<f64>> for Interval<f32> {
    type Error = ArithmeticFailure;
    fn try_from(value: Interval<f64>) -> Result<Self, Self::Error> {
        if value.lo < -f64::from(f32::MAX) || value.hi > f64::from(f32::MAX) {
            return Err(ArithmeticFailure::Overflow);
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "range checked and endpoints are stepped outward"
        )]
        let lo = value.lo as f32;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "range checked and endpoints are stepped outward"
        )]
        let hi = value.hi as f32;
        let lower = if f64::from(lo) > value.lo {
            lo.next_down()
        } else {
            lo
        };
        let upper = if f64::from(hi) < value.hi {
            hi.next_up()
        } else {
            hi
        };
        if !lower.is_finite() || !upper.is_finite() {
            return Err(ArithmeticFailure::Overflow);
        }
        Self::new(lower, upper)
    }
}

#[cfg(test)]
/// Unit evidence for scalar enclosures.
///
/// TODO ´todo:code:add-typed-acceptance-and-rejection-cases´: add typed acceptance and rejection cases for measures, counts, affine roles and independently kinded uncertainty under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
mod tests {
    use super::{ArithmeticFailure, Interval, dot, frobenius_upper, sum_squares};

    /// Exact additions remain tight for both formats.
    ///
    /// TODO ´todo:code:test-compatible-quantity-addition-and-exact´: test compatible quantity addition and exact count arithmetic separately from ratio and affine-point rejection under rule:vision:semantic-kinds-and-roles.
    ///
    /// ´test:unit:exact-small-cases´
    #[test]
    fn exact_small_cases() {
        let a = Interval::point(1.0_f32).expect("finite");
        let b = Interval::point(2.0_f32).expect("finite");
        let c = a.checked_add(b).expect("sum");
        assert_eq!(c.lo().to_bits(), 3.0_f32.to_bits());
        assert_eq!(c.hi().to_bits(), 3.0_f32.to_bits());
        let a = Interval::point(1.0_f64).expect("finite");
        let b = Interval::point(2.0_f64).expect("finite");
        let c = a.checked_add(b).expect("sum");
        assert_eq!(c.lo().to_bits(), 3.0_f64.to_bits());
        assert_eq!(c.hi().to_bits(), 3.0_f64.to_bits());
    }

    /// Cancellation retains the lost unit.
    ///
    /// TODO ´todo:code:test-cancellation-with-carried-measurement-and´: test cancellation with carried measurement and computational uncertainty descriptors under def:vision:intrinsic-uncertainty-companions and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:cancellation-enclosed´
    #[test]
    fn cancellation_enclosed() {
        let a = Interval::point(1.0e8_f32).expect("finite");
        let b = Interval::point(1.0_f32).expect("finite");
        let c = a
            .checked_add(b)
            .expect("sum")
            .checked_sub(a)
            .expect("difference");
        assert!(c.contains(1.0));
        let a = Interval::point(1.0e16_f64).expect("finite");
        let b = Interval::point(1.0_f64).expect("finite");
        let c = a
            .checked_add(b)
            .expect("sum")
            .checked_sub(a)
            .expect("difference");
        assert!(c.contains(1.0));
    }

    /// The FMA residual exposes an inexact product.
    ///
    /// TODO ´todo:code:check-product-rounding-evidence-has-the´: check product rounding evidence has the product dimension and an uncertainty kind under rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:product-rounding-enclosed´
    #[test]
    fn product_rounding_enclosed() {
        let x = Interval::point(1.0_f32.next_up()).expect("finite");
        let y = x.checked_mul(x).expect("product");
        assert!(y.lo() < y.hi());
        let x = Interval::point(1.0_f64.next_up()).expect("finite");
        let y = x.checked_mul(x).expect("product");
        assert!(y.lo() < y.hi());
    }

    /// Subnormal results and the normal boundary use outward steps.
    ///
    /// TODO ´todo:code:exercise-unit-conversion-near-subnormal-boundaries´: exercise unit conversion near subnormal boundaries with outward uncertainty propagation under rule:vision:measure-conversions.
    ///
    /// ´test:unit:subnormal-boundaries-enclosed´
    #[test]
    fn subnormal_boundaries_enclosed() {
        let x = Interval::point(f32::MIN_POSITIVE).expect("finite");
        let y = x
            .checked_div(Interval::point(2.0).expect("finite"))
            .expect("quotient");
        assert!(y.lo() < y.hi());
        let x = Interval::point(f64::MIN_POSITIVE).expect("finite");
        let y = x
            .checked_div(Interval::point(2.0).expect("finite"))
            .expect("quotient");
        assert!(y.lo() < y.hi());
    }

    /// Overflow refuses a finite enclosure.
    ///
    /// TODO ´todo:code:verify-quantity-and-measure-conversion-overflow´: verify quantity and measure-conversion overflow refuses without dropping descriptors or uncertainty evidence under rule:vision:measure-conversions.
    ///
    /// ´test:unit:overflow-refuses´
    #[test]
    fn overflow_refuses() {
        let x = Interval::point(f32::MAX).expect("finite");
        assert!(matches!(x.checked_add(x), Err(ArithmeticFailure::Overflow)));
        assert!(matches!(x.checked_mul(x), Err(ArithmeticFailure::Overflow)));
        let x = Interval::point(f64::MAX).expect("finite");
        assert!(matches!(x.checked_add(x), Err(ArithmeticFailure::Overflow)));
        assert!(matches!(x.checked_mul(x), Err(ArithmeticFailure::Overflow)));
    }

    /// Invalid endpoints refuse construction.
    ///
    /// TODO ´todo:code:reject-invalid-quantity-roles-while-allowing´: reject invalid quantity roles while allowing non-integer count standard uncertainty with its own kind under def:vision:counts and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:invalid-input-refuses´
    #[test]
    fn invalid_input_refuses() {
        assert!(matches!(
            Interval::point(f32::NAN),
            Err(ArithmeticFailure::NonFiniteEndpoint)
        ));
        assert!(matches!(
            Interval::new(2.0_f32, 1.0),
            Err(ArithmeticFailure::InvertedEndpoints)
        ));
        assert!(matches!(
            Interval::point(f64::INFINITY),
            Err(ArithmeticFailure::NonFiniteEndpoint)
        ));
        assert!(matches!(
            Interval::new(2.0_f64, 1.0),
            Err(ArithmeticFailure::InvertedEndpoints)
        ));
    }

    /// A zero-containing divisor refuses division.
    ///
    /// TODO ´todo:code:verify-dimensioned-quotient-refusals-retain-the´: verify dimensioned quotient refusals retain the declared denominator and uncertainty contracts under req:vision:algebraic-unit-checks.
    ///
    /// ´test:unit:zero-divisor-refuses´
    #[test]
    fn zero_divisor_refuses() {
        let x = Interval::point(1.0_f32).expect("finite");
        let zero = Interval::new(-1.0_f32, 1.0).expect("finite");
        assert!(matches!(
            x.checked_div(zero),
            Err(ArithmeticFailure::DivisorContainsZero)
        ));
        let x = Interval::point(1.0_f64).expect("finite");
        let zero = Interval::new(-1.0_f64, 1.0).expect("finite");
        assert!(matches!(
            x.checked_div(zero),
            Err(ArithmeticFailure::DivisorContainsZero)
        ));
    }

    /// Widening is exact and narrowing contains the source.
    ///
    /// TODO ´todo:code:test-precision-and-measure-conversions-independently´: test precision and measure conversions independently with affine difference radii and unchanged semantic kinds under rule:vision:measure-conversions and rule:vision:precision-conversions.
    ///
    /// ´test:unit:precision-conversions-enclose´
    #[test]
    fn precision_conversions_enclose() {
        let x = Interval::point(0.1_f32).expect("finite");
        let wide: Interval<f64> = x.into();
        assert_eq!(wide.lo().to_bits(), f64::from(x.lo()).to_bits());
        let narrow = Interval::<f32>::try_from(wide).expect("range");
        assert!(narrow.contains(x.lo()));
        let x = Interval::point(0.1_f64).expect("finite");
        let narrow = Interval::<f32>::try_from(x).expect("range");
        assert!(f64::from(narrow.lo()) <= x.lo());
        assert!(f64::from(narrow.hi()) >= x.hi());
        let near_max = f64::from(f32::MAX).next_down();
        let near =
            Interval::<f32>::try_from(Interval::point(near_max).expect("finite")).expect("range");
        assert!(f64::from(near.lo()) <= near_max && f64::from(near.hi()) >= near_max);
        let beyond_max = f64::from(f32::MAX).next_up();
        assert!(matches!(
            Interval::<f32>::try_from(Interval::point(beyond_max).expect("finite")),
            Err(ArithmeticFailure::Overflow)
        ));
    }

    /// Scaling keeps a norm finite when direct squares overflow.
    ///
    /// TODO ´todo:code:test-norm-dimensions-and-restored-block´: test norm dimensions and restored block-bound measures after normalized scaling under rule:vision:heterogeneous-assembly and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:scaled-norm-encloses´
    #[test]
    fn scaled_norm_encloses() {
        let f = frobenius_upper(&[1.0e30_f32, 1.0e30_f32]).expect("finite norm");
        assert!(f.is_finite() && f >= 1.0e30_f32);
        let f = frobenius_upper(&[1.0e200_f64, 1.0e200_f64]).expect("finite norm");
        assert!(f.is_finite() && f >= 1.0e200_f64);
    }

    /// Plain-slice reductions enclose their exact value and reject unequal lengths.
    ///
    /// TODO ´todo:code:test-typed-reductions-reject-incompatible-summands´: test typed reductions reject incompatible summands and preserve product or squared uncertainty dimensions under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:reductions-enclose´
    #[test]
    fn reductions_enclose() {
        assert!(
            dot(&[1.0_f32, 2.0], &[2.0, 3.0])
                .expect("dot")
                .contains(8.0)
        );
        assert!(sum_squares(&[1.0_f32, 2.0]).expect("squares").contains(5.0));
        assert!(
            dot(&[1.0_f64, 2.0], &[2.0, 3.0])
                .expect("dot")
                .contains(8.0)
        );
        assert!(sum_squares(&[1.0_f64, 2.0]).expect("squares").contains(5.0));
        assert!(matches!(
            dot(&[1.0_f64], &[]),
            Err(ArithmeticFailure::LengthMismatch)
        ));
    }
}
