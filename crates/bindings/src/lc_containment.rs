// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite imports and directed native comparisons for LC family strata.
//! This checker is numerical only: its caller must authenticate the original enclosure.

use nrz_cas::{Fault, Rational, limits::Budget};
use std::cmp::Ordering;

/// Distinct native arithmetic contracts; root arithmetic revisions do not imply these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcContainmentContract {
    /// `lc-containment-binary32@1`.
    Binary32,
    /// `lc-containment-binary64@1`.
    Binary64,
}

impl LcContainmentContract {
    /// The closed revision identifier retained by an LC application.
    #[must_use]
    pub const fn revision(self) -> &'static str {
        match self {
            Self::Binary32 => "lc-containment-binary32@1",
            Self::Binary64 => "lc-containment-binary64@1",
        }
    }
}

/// Explicit arithmetic premises for the containment revision.
#[expect(
    clippy::struct_excessive_bools,
    reason = "five independent arithmetic premises must each refuse when false"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LcArithmeticMode {
    /// Correctly rounded ties-to-even primitives.
    pub nearest_even: bool,
    /// Subnormal results are preserved.
    pub gradual_underflow: bool,
    /// No unrecorded fused operation or reassociation.
    pub strict_operations: bool,
    /// No extended intermediate precision.
    pub native_intermediates: bool,
    /// Each outward endpoint has a directed proof.
    pub directed_endpoints: bool,
}

impl LcArithmeticMode {
    /// All premises explicitly checked by the owner of the numerical implementation.
    pub const REQUIRED: Self = Self {
        nearest_even: true,
        gradual_underflow: true,
        strict_operations: true,
        native_intermediates: true,
        directed_endpoints: true,
    };
}

/// A finite face and its checked inward threshold.
#[derive(Clone, Debug)]
pub struct LcFace<T> {
    /// Exact rational boundary.
    pub exact: Rational,
    /// Candidate `ceil_format` for a lower face or `floor_format` for an upper face.
    pub inward: T,
    /// Strict mathematical face.
    pub open: bool,
}

/// A one-sided or two-sided exact stratum; `None` means no finite face.
#[derive(Clone, Debug)]
pub struct LcStratum<T> {
    /// Optional finite lower face.
    pub lower: Option<LcFace<T>>,
    /// Optional finite upper face.
    pub upper: Option<LcFace<T>>,
}

/// Refusal occurs before a native comparison can assert containment.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LcContainmentError {
    /// Unsupported arithmetic mode or missing directed proof.
    #[error("unsupported LC containment arithmetic mode")]
    UnsupportedMode,
    /// A NaN, infinity, inverted enclosure or absent required finite threshold.
    #[error("invalid finite LC containment input")]
    InvalidInput,
    /// A proposed threshold is not the nearest representable inward value.
    #[error("LC threshold is not directed inward")]
    Threshold,
    /// The presented endpoints do not cover the authenticated exact image.
    #[error("LC endpoints do not enclose the exact image")]
    EnclosurePremise,
    /// Exact rational arithmetic exhausted the caller ledger.
    #[error(transparent)]
    Exact(#[from] Fault),
}

/// Independently decode one finite binary32 bit pattern; the caller retains its bits.
/// # Errors
/// Refuses NaN, infinity or exhausted exact arithmetic.
pub fn decode_binary32(bits: u32, budget: &mut Budget) -> Result<Rational, LcContainmentError> {
    if bits & 0x7f80_0000 == 0x7f80_0000 {
        return Err(LcContainmentError::InvalidInput);
    }
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    let significand = if exponent == 0 {
        fraction
    } else {
        fraction | (1 << 23)
    };
    let power = if exponent == 0 {
        -149
    } else {
        i64::from(exponent) - 150
    };
    dyadic(i64::from(significand), power, bits >> 31 != 0, budget)
}

/// Independently decode one finite binary64 bit pattern; the caller retains its bits.
/// # Errors
/// Refuses NaN, infinity or exhausted exact arithmetic.
pub fn decode_binary64(bits: u64, budget: &mut Budget) -> Result<Rational, LcContainmentError> {
    if bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000 {
        return Err(LcContainmentError::InvalidInput);
    }
    let exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let significand = if exponent == 0 {
        fraction
    } else {
        fraction | (1 << 52)
    };
    let power = if exponent == 0 {
        -1074
    } else {
        i64::try_from(exponent).map_err(|_| LcContainmentError::InvalidInput)? - 1075
    };
    dyadic(
        i64::try_from(significand).map_err(|_| LcContainmentError::InvalidInput)?,
        power,
        bits >> 63 != 0,
        budget,
    )
}

/// Decode one finite signed dyadic with bounded exact exponent steps.
fn dyadic(
    significand: i64,
    power: i64,
    negative: bool,
    budget: &mut Budget,
) -> Result<Rational, LcContainmentError> {
    let integer =
        Rational::from_ratio(if negative { -significand } else { significand }, 1, budget)?;
    let two = Rational::from_ratio(2, 1, budget)?;
    let mut scale = Rational::one();
    let mut remaining = power;
    while remaining != 0 {
        let step = remaining.clamp(-256, 256);
        scale = scale.mul(&two.pow(step, budget)?, budget)?;
        remaining -= step;
    }
    Ok(integer.mul(&scale, budget)?)
}

/// Compare exact decoded values on the caller ledger.
fn cmp(a: &Rational, b: &Rational, budget: &mut Budget) -> Result<Ordering, LcContainmentError> {
    Ok(a.compare(b, budget)?)
}

/// Adjacent finite binary32 value in the chosen direction, if one exists.
fn neighbor32(bits: u32, up: bool) -> Option<u32> {
    let value = f32::from_bits(bits);
    if !value.is_finite() {
        return None;
    }
    if up {
        if bits == 0x7f7f_ffff {
            return None;
        }
        Some(if value == 0.0 {
            1
        } else if value < 0.0 {
            bits - 1
        } else {
            bits + 1
        })
    } else {
        if bits == 0xff7f_ffff {
            return None;
        }
        Some(if value == 0.0 {
            0x8000_0001
        } else if value < 0.0 {
            bits + 1
        } else {
            bits - 1
        })
    }
}

/// Adjacent finite binary64 value in the chosen direction, if one exists.
fn neighbor64(bits: u64, up: bool) -> Option<u64> {
    let value = f64::from_bits(bits);
    if !value.is_finite() {
        return None;
    }
    if up {
        if bits == 0x7fef_ffff_ffff_ffff {
            return None;
        }
        Some(if value == 0.0 {
            1
        } else if value < 0.0 {
            bits - 1
        } else {
            bits + 1
        })
    } else {
        if bits == 0xffef_ffff_ffff_ffff {
            return None;
        }
        Some(if value == 0.0 {
            0x8000_0000_0000_0001
        } else if value < 0.0 {
            bits + 1
        } else {
            bits - 1
        })
    }
}

/// Verify that the candidate is the nearest inward representable face.
fn face(
    exact: &Rational,
    threshold: &Rational,
    neighbor: Option<Rational>,
    lower: bool,
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    let inward = cmp(exact, threshold, budget)?;
    let adjacent = neighbor
        .map(|value| cmp(&value, exact, budget))
        .transpose()?;
    Ok(if lower {
        inward != Ordering::Greater && adjacent.is_none_or(|order| order == Ordering::Less)
    } else {
        inward != Ordering::Less && adjacent.is_none_or(|order| order == Ordering::Greater)
    })
}

/// Apply the exact strict or closed face comparison.
fn boundary(
    endpoint: &Rational,
    face: &Rational,
    exact: &Rational,
    lower: bool,
    open: bool,
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    let order = cmp(endpoint, face, budget)?;
    let strict_inward = if lower {
        cmp(exact, face, budget)? == Ordering::Less
    } else {
        cmp(face, exact, budget)? == Ordering::Less
    };
    Ok(if lower {
        order == Ordering::Greater || (order == Ordering::Equal && (!open || strict_inward))
    } else {
        order == Ordering::Less || (order == Ordering::Equal && (!open || strict_inward))
    })
}

/// Validate the finite inward threshold before comparing any endpoint.
fn validated_face32(
    face_input: &LcFace<f32>,
    lower: bool,
    budget: &mut Budget,
) -> Result<Rational, LcContainmentError> {
    let bits = face_input.inward.to_bits();
    let threshold = decode_binary32(bits, budget)?;
    let adjacent = neighbor32(bits, !lower)
        .map(|bits| decode_binary32(bits, budget))
        .transpose()?;
    if !face(&face_input.exact, &threshold, adjacent, lower, budget)? {
        return Err(LcContainmentError::Threshold);
    }
    Ok(threshold)
}

/// Check a binary32 enclosure against independently decoded exact image and faces.
/// The image must already be authenticated as covering every original admissible source.
/// # Errors
/// Refuses unsupported modes, invalid data, bad thresholds, failed enclosure premises and exact budget failure.
pub fn contains_binary32(
    endpoints: [f32; 2],
    image: [&Rational; 2],
    stratum: &LcStratum<f32>,
    mode: LcArithmeticMode,
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    if mode != LcArithmeticMode::REQUIRED {
        return Err(LcContainmentError::UnsupportedMode);
    }
    let l = decode_binary32(endpoints[0].to_bits(), budget)?;
    let u = decode_binary32(endpoints[1].to_bits(), budget)?;
    if cmp(&l, &u, budget)? == Ordering::Greater
        || cmp(image[0], image[1], budget)? == Ordering::Greater
    {
        return Err(LcContainmentError::InvalidInput);
    }
    let lower = stratum
        .lower
        .as_ref()
        .map(|f| validated_face32(f, true, budget))
        .transpose()?;
    let upper = stratum
        .upper
        .as_ref()
        .map(|f| validated_face32(f, false, budget))
        .transpose()?;
    if cmp(&l, image[0], budget)? == Ordering::Greater
        || cmp(image[1], &u, budget)? == Ordering::Greater
    {
        return Err(LcContainmentError::EnclosurePremise);
    }
    let lower_ok = match (stratum.lower.as_ref(), lower.as_ref()) {
        (Some(f), Some(t)) => boundary(&l, t, &f.exact, true, f.open, budget)?,
        _ => true,
    };
    let upper_ok = match (stratum.upper.as_ref(), upper.as_ref()) {
        (Some(f), Some(t)) => boundary(&u, t, &f.exact, false, f.open, budget)?,
        _ => true,
    };
    Ok(lower_ok && upper_ok)
}

/// Validate the finite inward threshold before comparing any endpoint.
fn validated_face64(
    face_input: &LcFace<f64>,
    lower: bool,
    budget: &mut Budget,
) -> Result<Rational, LcContainmentError> {
    let bits = face_input.inward.to_bits();
    let threshold = decode_binary64(bits, budget)?;
    let adjacent = neighbor64(bits, !lower)
        .map(|bits| decode_binary64(bits, budget))
        .transpose()?;
    if !face(&face_input.exact, &threshold, adjacent, lower, budget)? {
        return Err(LcContainmentError::Threshold);
    }
    Ok(threshold)
}

/// Check the independent binary64 contract with binary64 decoding and adjacency.
/// # Errors
/// Refuses unsupported modes, invalid data, bad thresholds, failed enclosure premises and exact budget failure.
pub fn contains_binary64(
    endpoints: [f64; 2],
    image: [&Rational; 2],
    stratum: &LcStratum<f64>,
    mode: LcArithmeticMode,
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    if mode != LcArithmeticMode::REQUIRED {
        return Err(LcContainmentError::UnsupportedMode);
    }
    let l = decode_binary64(endpoints[0].to_bits(), budget)?;
    let u = decode_binary64(endpoints[1].to_bits(), budget)?;
    if cmp(&l, &u, budget)? == Ordering::Greater
        || cmp(image[0], image[1], budget)? == Ordering::Greater
    {
        return Err(LcContainmentError::InvalidInput);
    }
    let lower = stratum
        .lower
        .as_ref()
        .map(|f| validated_face64(f, true, budget))
        .transpose()?;
    let upper = stratum
        .upper
        .as_ref()
        .map(|f| validated_face64(f, false, budget))
        .transpose()?;
    if cmp(&l, image[0], budget)? == Ordering::Greater
        || cmp(image[1], &u, budget)? == Ordering::Greater
    {
        return Err(LcContainmentError::EnclosurePremise);
    }
    let lower_ok = match (stratum.lower.as_ref(), lower.as_ref()) {
        (Some(f), Some(t)) => boundary(&l, t, &f.exact, true, f.open, budget)?,
        _ => true,
    };
    let upper_ok = match (stratum.upper.as_ref(), upper.as_ref()) {
        (Some(f), Some(t)) => boundary(&u, t, &f.exact, false, f.open, budget)?,
        _ => true,
    };
    Ok(lower_ok && upper_ok)
}
/// Half the binary32 subnormal quantum, the worst ties-to-even underflow rounding term.
/// It applies only to a proved correctly rounded primitive in gradual-underflow mode.
/// # Errors
/// Refuses exhausted exact arithmetic.
pub fn underflow_bound_binary32(budget: &mut Budget) -> Result<Rational, LcContainmentError> {
    dyadic(1, -150, false, budget)
}

/// Half the binary64 subnormal quantum under the same arithmetic premises.
/// # Errors
/// Refuses exhausted exact arithmetic.
pub fn underflow_bound_binary64(budget: &mut Budget) -> Result<Rational, LcContainmentError> {
    dyadic(1, -1075, false, budget)
}

/// Prove a proposed binary32 outward box contains the exact center plus or minus total radius.
/// The numerical companion is already included in `total` and is never added again.
/// # Errors
/// Refuses negative total, nonfinite/inverted endpoints or exhausted exact arithmetic.
pub fn outward_total_binary32(
    center: f32,
    total: f32,
    endpoints: [f32; 2],
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    let c = decode_binary32(center.to_bits(), budget)?;
    let t = decode_binary32(total.to_bits(), budget)?;
    if t.is_negative() {
        return Err(LcContainmentError::InvalidInput);
    }
    let l = decode_binary32(endpoints[0].to_bits(), budget)?;
    let u = decode_binary32(endpoints[1].to_bits(), budget)?;
    if cmp(&l, &u, budget)? == Ordering::Greater {
        return Err(LcContainmentError::InvalidInput);
    }
    Ok(cmp(&l, &c.sub(&t, budget)?, budget)? != Ordering::Greater
        && cmp(&c.add(&t, budget)?, &u, budget)? != Ordering::Greater)
}

/// Prove a proposed binary64 outward box contains the exact center plus or minus total radius.
/// # Errors
/// Refuses negative total, nonfinite/inverted endpoints or exhausted exact arithmetic.
pub fn outward_total_binary64(
    center: f64,
    total: f64,
    endpoints: [f64; 2],
    budget: &mut Budget,
) -> Result<bool, LcContainmentError> {
    let c = decode_binary64(center.to_bits(), budget)?;
    let t = decode_binary64(total.to_bits(), budget)?;
    if t.is_negative() {
        return Err(LcContainmentError::InvalidInput);
    }
    let l = decode_binary64(endpoints[0].to_bits(), budget)?;
    let u = decode_binary64(endpoints[1].to_bits(), budget)?;
    if cmp(&l, &u, budget)? == Ordering::Greater {
        return Err(LcContainmentError::InvalidInput);
    }
    Ok(cmp(&l, &c.sub(&t, budget)?, budget)? != Ordering::Greater
        && cmp(&c.add(&t, budget)?, &u, budget)? != Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nrz_cas::limits::Limits;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn budget() -> Result<Budget, Fault> {
        Budget::new(Limits::default())
    }
    fn q(n: i64, d: u64, b: &mut Budget) -> Result<Rational, Fault> {
        Rational::from_ratio(n, d, b)
    }

    fn expected_max(significand: i64, exponent: i64, b: &mut Budget) -> Result<Rational, Fault> {
        let mut value = q(significand, 1, b)?;
        let two = q(2, 1, b)?;
        let mut remaining = exponent;
        while remaining > 0 {
            let step = remaining.min(256);
            value = value.mul(&two.pow(step, b)?, b)?;
            remaining -= step;
        }
        Ok(value)
    }

    macro_rules! format_controls {
        ($thresholds:ident, $edges:ident, $premises:ident,
         $ty:ty, $contains:ident, $decode:ident, $one:expr, $up:expr,
         $down:expr, $max:expr, $min:expr, $inf:expr, $sig:expr, $exp:expr) => {
            #[test]
            fn $thresholds() -> TestResult {
                let b = &mut budget()?;
                let one = Rational::one();
                let two = q(2, 1, b)?;
                for (lower, candidate) in [(true, $up), (true, $down), (false, $down), (false, $up)]
                {
                    let face = LcFace {
                        exact: one.clone(),
                        inward: candidate,
                        open: false,
                    };
                    let stratum = LcStratum {
                        lower: lower.then_some(face.clone()),
                        upper: (!lower).then_some(face),
                    };
                    assert!(matches!(
                        $contains(
                            [$one, $one],
                            [&one, &one],
                            &stratum,
                            LcArithmeticMode::REQUIRED,
                            b
                        ),
                        Err(LcContainmentError::Threshold)
                    ));
                }
                let bad_upper = LcFace {
                    exact: one.clone(),
                    inward: $up,
                    open: false,
                };
                let failed_lower = LcFace {
                    exact: two.clone(),
                    inward: 2.0 as $ty,
                    open: false,
                };
                let both = LcStratum {
                    lower: Some(failed_lower),
                    upper: Some(bad_upper.clone()),
                };
                assert!(matches!(
                    $contains(
                        [$one, $one],
                        [&one, &one],
                        &both,
                        LcArithmeticMode::REQUIRED,
                        b
                    ),
                    Err(LcContainmentError::Threshold)
                ));
                let coverage = LcStratum {
                    lower: None,
                    upper: Some(bad_upper),
                };
                assert!(matches!(
                    $contains(
                        [$one, $one],
                        [&one, &two],
                        &coverage,
                        LcArithmeticMode::REQUIRED,
                        b
                    ),
                    Err(LcContainmentError::Threshold)
                ));
                Ok(())
            }

            #[test]
            fn $edges() -> TestResult {
                let b = &mut budget()?;
                let negative = q(-1, 1, b)?;
                let max = expected_max($sig, $exp, b)?;
                assert_eq!($decode($max.to_bits(), b)?, max);
                let min = max.negated(b)?;
                assert_eq!($decode($min.to_bits(), b)?, min);
                for (value, exact) in [(-1.0 as $ty, negative), ($max, max), ($min, min)] {
                    for lower in [true, false] {
                        let face = LcFace {
                            exact: exact.clone(),
                            inward: value,
                            open: false,
                        };
                        let stratum = LcStratum {
                            lower: lower.then_some(face.clone()),
                            upper: (!lower).then_some(face),
                        };
                        assert!($contains(
                            [value, value],
                            [&exact, &exact],
                            &stratum,
                            LcArithmeticMode::REQUIRED,
                            b
                        )?);
                    }
                }
                Ok(())
            }

            #[test]
            fn $premises() -> TestResult {
                let b = &mut budget()?;
                let zero = Rational::zero();
                let one = Rational::one();
                let two = q(2, 1, b)?;
                let none = LcStratum::<$ty> {
                    lower: None,
                    upper: None,
                };
                for image in [[&zero, &one], [&one, &two]] {
                    assert!(matches!(
                        $contains([$one, $one], image, &none, LcArithmeticMode::REQUIRED, b),
                        Err(LcContainmentError::EnclosurePremise)
                    ));
                }
                assert!(matches!(
                    $contains(
                        [2.0 as $ty, $one],
                        [&one, &one],
                        &none,
                        LcArithmeticMode::REQUIRED,
                        b
                    ),
                    Err(LcContainmentError::InvalidInput)
                ));
                assert!(matches!(
                    $contains(
                        [$one, $one],
                        [&two, &one],
                        &none,
                        LcArithmeticMode::REQUIRED,
                        b
                    ),
                    Err(LcContainmentError::InvalidInput)
                ));
                for lower in [true, false] {
                    let face = LcFace {
                        exact: one.clone(),
                        inward: $inf,
                        open: false,
                    };
                    let stratum = LcStratum {
                        lower: lower.then_some(face.clone()),
                        upper: (!lower).then_some(face),
                    };
                    assert!(matches!(
                        $contains(
                            [$one, $one],
                            [&one, &one],
                            &stratum,
                            LcArithmeticMode::REQUIRED,
                            b
                        ),
                        Err(LcContainmentError::InvalidInput)
                    ));
                }
                let upper = LcStratum {
                    lower: None,
                    upper: Some(LcFace {
                        exact: one.clone(),
                        inward: $one,
                        open: false,
                    }),
                };
                let above = $decode($up.to_bits(), b)?;
                assert!(!$contains(
                    [$up, $up],
                    [&above, &above],
                    &upper,
                    LcArithmeticMode::REQUIRED,
                    b
                )?);
                let lower = LcStratum {
                    lower: Some(LcFace {
                        exact: one,
                        inward: $one,
                        open: false,
                    }),
                    upper: None,
                };
                let below = $decode($down.to_bits(), b)?;
                assert!(!$contains(
                    [$down, $down],
                    [&below, &below],
                    &lower,
                    LcArithmeticMode::REQUIRED,
                    b
                )?);
                Ok(())
            }
        };
    }

    format_controls!(
        lc_binary32_threshold_refusals,
        lc_binary32_edge_faces,
        lc_binary32_premise_refusals,
        f32,
        contains_binary32,
        decode_binary32,
        1.0_f32,
        f32::from_bits(1.0_f32.to_bits() + 1),
        f32::from_bits(1.0_f32.to_bits() - 1),
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        16_777_215,
        104
    );
    format_controls!(
        lc_binary64_threshold_refusals,
        lc_binary64_edge_faces,
        lc_binary64_premise_refusals,
        f64,
        contains_binary64,
        decode_binary64,
        1.0_f64,
        f64::from_bits(1.0_f64.to_bits() + 1),
        f64::from_bits(1.0_f64.to_bits() - 1),
        f64::MAX,
        -f64::MAX,
        f64::INFINITY,
        9_007_199_254_740_991,
        971
    );

    #[test]
    fn lc_containment_revision_identifiers() {
        assert_eq!(
            LcContainmentContract::Binary32.revision(),
            "lc-containment-binary32@1"
        );
        assert_eq!(
            LcContainmentContract::Binary64.revision(),
            "lc-containment-binary64@1"
        );
    }

    #[test]
    fn lc_containment_binary32_exact_and_adjacent_faces() -> TestResult {
        let b = &mut budget()?;
        let zero = Rational::zero();
        let threshold = 1.5_f32;
        let exact_threshold = q(3, 2, b)?;
        let face = LcFace {
            exact: exact_threshold.clone(),
            inward: threshold,
            open: true,
        };
        let stratum = LcStratum {
            lower: Some(face),
            upper: None,
        };
        assert!(!contains_binary32(
            [threshold, threshold],
            [&exact_threshold, &exact_threshold],
            &stratum,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        let inner = f32::from_bits(threshold.to_bits() + 1);
        let image = decode_binary32(inner.to_bits(), b)?;
        assert!(contains_binary32(
            [inner, inner],
            [&image, &image],
            &stratum,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        assert_eq!(decode_binary32(0x8000_0000, b)?, zero);
        assert_eq!(
            decode_binary32(1, b)?,
            Rational::from_ratio(2, 1, b)?.pow(-149, b)?
        );
        assert_eq!(
            decode_binary32(f32::MAX.to_bits(), b)?.component_bits(),
            (128, 1)
        );
        assert!(decode_binary32(f32::INFINITY.to_bits(), b).is_err());
        let midpoint = q(16_777_217, 16_777_216, b)?;
        let ceil = f32::from_bits(1.0_f32.to_bits() + 1);
        let inward = LcStratum {
            lower: Some(LcFace {
                exact: midpoint.clone(),
                inward: ceil,
                open: true,
            }),
            upper: None,
        };
        let exact_ceil = decode_binary32(ceil.to_bits(), b)?;
        assert!(contains_binary32(
            [ceil, ceil],
            [&exact_ceil, &exact_ceil],
            &inward,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        let floor = LcStratum {
            lower: None,
            upper: Some(LcFace {
                exact: midpoint,
                inward: 1.0,
                open: true,
            }),
        };
        let one = Rational::one();
        assert!(contains_binary32(
            [1.0, 1.0],
            [&one, &one],
            &floor,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        let closed_zero = LcStratum {
            lower: Some(LcFace {
                exact: zero.clone(),
                inward: 0.0,
                open: false,
            }),
            upper: None,
        };
        assert!(contains_binary32(
            [-0.0, 0.0],
            [&zero, &zero],
            &closed_zero,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        assert!(matches!(
            contains_binary32(
                [f32::INFINITY, 0.0],
                [&zero, &zero],
                &closed_zero,
                LcArithmeticMode::REQUIRED,
                b
            ),
            Err(LcContainmentError::InvalidInput)
        ));
        Ok(())
    }

    #[test]
    fn lc_containment_binary64_exact_and_adjacent_faces() -> TestResult {
        let b = &mut budget()?;
        let one = Rational::one();
        let half = q(1, 2, b)?;
        let above = f64::from_bits(1.0_f64.to_bits() + 1);
        let stratum = LcStratum {
            lower: Some(LcFace {
                exact: half,
                inward: 1.0,
                open: true,
            }),
            upper: None,
        };
        assert!(matches!(
            contains_binary64(
                [1.0, 1.0],
                [&one, &one],
                &stratum,
                LcArithmeticMode::REQUIRED,
                b
            ),
            Err(LcContainmentError::Threshold)
        ));
        let exact_above = decode_binary64(above.to_bits(), b)?;
        let stratum = LcStratum {
            lower: Some(LcFace {
                exact: one.clone(),
                inward: 1.0,
                open: true,
            }),
            upper: None,
        };
        assert!(!contains_binary64(
            [1.0, 1.0],
            [&one, &one],
            &stratum,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        assert!(contains_binary64(
            [above, above],
            [&exact_above, &exact_above],
            &stratum,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        assert_eq!(decode_binary64(0x8000_0000_0000_0000, b)?, Rational::zero());
        let subnormal = decode_binary64(1, b)?;
        assert_eq!(subnormal.component_bits(), (1, 1075));
        assert_eq!(
            decode_binary64(f64::MAX.to_bits(), b)?.component_bits(),
            (1024, 1)
        );
        assert!(decode_binary64(f64::NAN.to_bits(), b).is_err());
        binary64_face_edges(b, &one, above, &exact_above)?;
        Ok(())
    }

    fn binary64_face_edges(
        b: &mut Budget,
        one: &Rational,
        above: f64,
        exact_above: &Rational,
    ) -> TestResult {
        let midpoint = q(9_007_199_254_740_993, 9_007_199_254_740_992, b)?;
        let inward = LcStratum {
            lower: Some(LcFace {
                exact: midpoint.clone(),
                inward: above,
                open: true,
            }),
            upper: None,
        };
        assert!(contains_binary64(
            [above, above],
            [exact_above, exact_above],
            &inward,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        let floor = LcStratum {
            lower: None,
            upper: Some(LcFace {
                exact: midpoint,
                inward: 1.0,
                open: true,
            }),
        };
        assert!(contains_binary64(
            [1.0, 1.0],
            [one, one],
            &floor,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        let zero = Rational::zero();
        let closed_zero = LcStratum {
            lower: Some(LcFace {
                exact: zero.clone(),
                inward: -0.0,
                open: false,
            }),
            upper: None,
        };
        assert!(contains_binary64(
            [-0.0, 0.0],
            [&zero, &zero],
            &closed_zero,
            LcArithmeticMode::REQUIRED,
            b
        )?);
        assert!(matches!(
            contains_binary64(
                [f64::NEG_INFINITY, 0.0],
                [&zero, &zero],
                &closed_zero,
                LcArithmeticMode::REQUIRED,
                b
            ),
            Err(LcContainmentError::InvalidInput)
        ));
        Ok(())
    }

    #[test]
    fn lc_containment_modes_and_error_scope() -> TestResult {
        let b = &mut budget()?;
        let one = Rational::one();
        let stratum = LcStratum::<f32> {
            lower: None,
            upper: None,
        };
        for disabled in 0..5 {
            let mut mode = LcArithmeticMode::REQUIRED;
            match disabled {
                0 => mode.nearest_even = false,
                1 => mode.gradual_underflow = false,
                2 => mode.strict_operations = false,
                3 => mode.native_intermediates = false,
                _ => mode.directed_endpoints = false,
            }
            assert!(matches!(
                contains_binary32([1.0, 1.0], [&one, &one], &stratum, mode, b),
                Err(LcContainmentError::UnsupportedMode)
            ));
            let stratum64 = LcStratum::<f64> {
                lower: None,
                upper: None,
            };
            assert!(matches!(
                contains_binary64([1.0, 1.0], [&one, &one], &stratum64, mode, b),
                Err(LcContainmentError::UnsupportedMode)
            ));
        }
        assert_eq!(underflow_bound_binary32(b)?.component_bits(), (1, 151));
        assert_eq!(underflow_bound_binary64(b)?.component_bits(), (1, 1076));
        let quantum32 = f32::from_bits(1);
        let quantum64 = f64::from_bits(1);
        assert!(outward_total_binary32(
            0.0,
            quantum32,
            [-quantum32, quantum32],
            b
        )?);
        assert!(!outward_total_binary32(
            0.0,
            quantum32,
            [0.0, quantum32],
            b
        )?);
        assert!(outward_total_binary64(
            0.0,
            quantum64,
            [-quantum64, quantum64],
            b
        )?);
        assert!(!outward_total_binary64(
            0.0,
            quantum64,
            [0.0, quantum64],
            b
        )?);
        assert!(matches!(
            outward_total_binary32(0.0, -quantum32, [-quantum32, quantum32], b),
            Err(LcContainmentError::InvalidInput)
        ));
        assert!(matches!(
            outward_total_binary64(0.0, -quantum64, [-quantum64, quantum64], b),
            Err(LcContainmentError::InvalidInput)
        ));
        Ok(())
    }
}
