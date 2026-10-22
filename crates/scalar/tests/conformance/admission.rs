// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`finite_format_admission`] | scalar | Canonicalization, adjacency and format identity are visible through the facade. |
//! | [`exact_identity_shortcuts`] | scalar | Only proved identities retain singleton endpoints. |
//! | [`primitive_domain_and_range_refusals`] | scalar | Both formats reject undefined real operations and nonfinite enclosures. |

//! Public input, identity, domain and range contracts.

use crate::testing::{TestResult, point};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Precision, Scalar};

/// Canonicalization, adjacency and format identity are visible through the facade.
/// ´claim:scalar:finite-format-admission´
/// ´test:integration:finite-format-admission´
#[test]
fn finite_format_admission() -> TestResult {
    assert_eq!(f32::PRECISION, Precision::F32);
    assert_eq!(f64::PRECISION, Precision::F64);
    assert_eq!(Finite::new(-0.0_f32)?.get().to_bits(), 0);
    assert_eq!(Finite::new(-0.0_f64)?.get().to_bits(), 0);
    assert_eq!(Finite::<f32>::ZERO.next_up()?.get().to_bits(), 1);
    assert_eq!(
        Finite::<f64>::ZERO.next_down()?.get().to_bits(),
        0x8000_0000_0000_0001
    );
    assert_eq!(
        Finite::new(f32::MAX)?.next_up(),
        Err(ArithmeticFailure::Overflow)
    );
    assert_eq!(
        Finite::new(-f64::MAX)?.next_down(),
        Err(ArithmeticFailure::Overflow)
    );
    assert_eq!(Finite::new(f32::NAN), Err(ArithmeticFailure::NonFinite));
    assert_eq!(
        Finite::new(f64::INFINITY),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        Finite::new(f64::NEG_INFINITY),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        Enclosure::new(2.0_f32, 1.0),
        Err(ArithmeticFailure::InvertedEndpoints)
    );
    assert_eq!(
        Enclosure::new(0.0_f64, f64::NAN),
        Err(ArithmeticFailure::NonFinite)
    );
    Ok(())
}

/// Each admitted identity stays exact, including finite extremes and signed zero.
fn identities<P: Scalar>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let zero = point(P::ZERO)?;
    let one = point(P::ONE)?;
    let negative = point(-P::ONE)?;
    for raw in [P::ZERO, P::ZERO.next_up(), P::ONE, -P::ONE, P::max_finite()] {
        let value = point(raw)?;
        assert_eq!(value.add(zero, &context)?, value);
        assert_eq!(zero.add(value, &context)?, value);
        assert_eq!(value.sub(value, &context)?, zero);
        assert_eq!(value.mul(zero, &context)?, zero);
        assert_eq!(value.mul(one, &context)?, value);
        assert_eq!(value.mul(negative, &context)?, value.negated());
        assert_eq!(value.div(one, &context)?, value);
        assert_eq!(value.div(negative, &context)?, value.negated());
        if raw != P::ZERO {
            assert_eq!(zero.div(value, &context)?, zero);
        }
    }
    assert_eq!(zero.sqrt(&context)?, zero);
    assert_eq!(one.sqrt(&context)?, one);
    assert_eq!(negative.abs(), one);
    Ok(())
}

/// Only proved identities retain singleton endpoints.
/// ´claim:scalar:exact-identity-shortcuts´
/// ´test:integration:exact-identity-shortcuts´
#[test]
fn exact_identity_shortcuts() -> TestResult {
    identities::<f32>()?;
    identities::<f64>()
}

/// Domain failures precede arithmetic, while finite-neighbour overflow is explicit.
fn refusals<P: Scalar>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let one = point(P::ONE)?;
    let maximum = point(P::max_finite())?;
    let crossing = Enclosure::new(-P::ONE, P::ONE)?;
    assert_eq!(
        one.div(crossing, &context),
        Err(ArithmeticFailure::ZeroInDivisor)
    );
    assert_eq!(
        crossing.sqrt(&context),
        Err(ArithmeticFailure::NegativeRoot)
    );
    assert_eq!(
        maximum.add(maximum, &context),
        Err(ArithmeticFailure::Overflow)
    );
    assert_eq!(
        maximum.mul(maximum, &context),
        Err(ArithmeticFailure::Overflow)
    );
    assert_eq!(maximum.add(one, &context), Err(ArithmeticFailure::Overflow));
    assert_eq!(
        maximum.negated().sub(one, &context),
        Err(ArithmeticFailure::Overflow)
    );
    Ok(())
}

/// Both formats reject undefined real operations and nonfinite enclosures.
/// ´claim:scalar:primitive-domain-and-range-refusals´
/// ´test:integration:primitive-domain-and-range-refusals´
#[test]
fn primitive_domain_and_range_refusals() -> TestResult {
    refusals::<f32>()?;
    refusals::<f64>()
}
