// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`literal_constant_rational_oracles`] | scalar | Mathematical descriptor constants and signed numeric grammar succeed in both formats. |
//! | [`literal_halfway_ties`] | scalar | Decimal halfway literals contain the exact ties and select the nearest even candidate. |
//! | [`literal_subnormal_and_underflow_oracles`] | scalar | Scientific notation reaches subnormals without forming an overflowing decimal denominator. |
//! | [`literal_syntax_refusals`] | scalar | Grammar rejects expressions, unsupported spellings and malformed fraction components. |
//! | [`literal_resource_boundaries`] | scalar | Limits are enforced at, below and above each resource boundary. |
//! | [`literal_range_and_divisor_refusals`] | scalar | Overflow and zero-containing denominator enclosures refuse even when an exact quotient is small. |

//! Exact rational expectations for bounded literal decoding.

use crate::testing::{Binary, Rational, TestResult, contains};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, LiteralLimits, NumericLiteral};
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::{One, Signed};

/// A literal's exact target and candidate displacement are checked without production decoding.
fn decoded<P: Binary>(text: &str, exact: &Rational) -> TestResult {
    let context = ArithmeticContext::new()?;
    let literal = NumericLiteral::new(text, LiteralLimits::default())?;
    let result = literal.decode::<P>(&context)?;
    contains(result.enclosure(), exact);
    let difference = (result.center().get().rational() - exact).abs();
    assert!(result.displacement().get().rational() >= difference);
    Ok(())
}

/// Mathematical descriptor constants and signed numeric grammar succeed in both formats.
/// ´claim:scalar:literal-constant-rational-oracles´
/// ´test:integration:literal-constant-rational-oracles´
#[test]
fn literal_constant_rational_oracles() -> TestResult {
    for (text, numerator, denominator) in [
        ("0", 0, 1),
        ("-0", 0, 1),
        ("1", 1, 1),
        ("+.5", 1, 2),
        ("1.", 1, 1),
        ("0.1", 1, 10),
        ("-1/10", -1, 10),
        ("1e-2", 1, 100),
        ("1.25E+2", 125, 1),
        ("5/9", 5, 9),
        ("45967/180", 45967, 180),
        ("5863/20", 5863, 20),
        ("273.15", 5463, 20),
        ("1/2500", 1, 2500),
        ("3/50000", 3, 50000),
        ("-3/-.5", 6, 1),
        ("1e-2/2e+1", 1, 2000),
    ] {
        let exact = Ratio::new(BigInt::from(numerator), BigInt::from(denominator));
        decoded::<f32>(text, &exact)?;
        decoded::<f64>(text, &exact)?;
    }
    Ok(())
}

/// Decimal halfway literals contain the exact ties and select the nearest even candidate.
/// ´claim:scalar:literal-halfway-ties´
/// ´test:integration:literal-halfway-ties´
#[test]
fn literal_halfway_ties() -> TestResult {
    let context = ArithmeticContext::new()?;
    let single = "1.000000059604644775390625";
    let exact_single = Ratio::new((BigInt::one() << 24) + 1, BigInt::one() << 24);
    decoded::<f32>(single, &exact_single)?;
    assert_eq!(
        NumericLiteral::new(single, LiteralLimits::default())?
            .decode::<f32>(&context)?
            .center()
            .get()
            .to_bits(),
        1.0_f32.to_bits()
    );
    let double = "1.00000000000000011102230246251565404236316680908203125";
    let exact_double = Ratio::new((BigInt::one() << 53) + 1, BigInt::one() << 53);
    decoded::<f64>(double, &exact_double)?;
    assert_eq!(
        NumericLiteral::new(double, LiteralLimits::default())?
            .decode::<f64>(&context)?
            .center()
            .get()
            .to_bits(),
        1.0_f64.to_bits()
    );
    Ok(())
}

/// Scientific notation reaches subnormals without forming an overflowing decimal denominator.
/// ´claim:scalar:literal-subnormal-and-underflow-oracles´
/// ´test:integration:literal-subnormal-and-underflow-oracles´
#[test]
fn literal_subnormal_and_underflow_oracles() -> TestResult {
    for (text, numerator, exponent) in [("1e-45", 1, 45), ("-1e-46", -1, 46)] {
        let exact = Ratio::new(BigInt::from(numerator), BigInt::from(10).pow(exponent));
        decoded::<f32>(text, &exact)?;
    }
    for (text, numerator, exponent) in [("5e-324", 5, 324), ("-1e-325", -1, 325)] {
        let exact = Ratio::new(BigInt::from(numerator), BigInt::from(10).pow(exponent));
        decoded::<f64>(text, &exact)?;
    }
    Ok(())
}

/// Grammar rejects expressions, unsupported spellings and malformed fraction components.
/// ´claim:scalar:literal-syntax-refusals´
/// ´test:integration:literal-syntax-refusals´
#[test]
fn literal_syntax_refusals() {
    for text in [
        "", ".", "+", "-", " 1", "1 ", "1_0", "NaN", "inf", "0x10", "1+2", "1/2/3", "/1", "1/",
        "--1", "1e", "1e+", "1ee2", "1.2.3", "one", "1/NaN",
    ] {
        assert!(
            matches!(
                NumericLiteral::new(text, LiteralLimits::default()),
                Err(ArithmeticFailure::InvalidLiteral)
            ),
            "{text}"
        );
    }
}

/// Limits are enforced at, below and above each resource boundary.
/// ´claim:scalar:literal-resource-boundaries´
/// ´test:integration:literal-resource-boundaries´
#[test]
fn literal_resource_boundaries() -> TestResult {
    let context = ArithmeticContext::new()?;
    assert!(matches!(
        NumericLiteral::new("1/2", LiteralLimits::new(1, 0, 100)),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    NumericLiteral::new("1/2", LiteralLimits::new(2, 0, 100))?.decode::<f64>(&context)?;
    assert!(matches!(
        NumericLiteral::new("1e2", LiteralLimits::new(1, 1, 100)),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    NumericLiteral::new("1e2", LiteralLimits::new(1, 2, 100))?.decode::<f64>(&context)?;
    assert!(matches!(
        NumericLiteral::new("1.0", LiteralLimits::new(2, 0, 100)),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    NumericLiteral::new("1.0", LiteralLimits::new(2, 1, 100))?.decode::<f64>(&context)?;
    for budget in [11, 12] {
        let result =
            NumericLiteral::new("1", LiteralLimits::new(1, 0, budget))?.decode::<f64>(&context)?;
        assert_eq!(result.work_used(), 11);
    }
    assert!(matches!(
        NumericLiteral::new("1", LiteralLimits::new(1, 0, 10))?.decode::<f64>(&context),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    assert!(matches!(
        NumericLiteral::new("1", LiteralLimits::new(1, 0, 7)),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    assert!(matches!(
        NumericLiteral::new(
            "1e9999999999999999999999",
            LiteralLimits::new(1, u32::MAX, 1000)
        ),
        Err(ArithmeticFailure::ResourceLimit)
    ));
    Ok(())
}

/// Overflow and zero-containing denominator enclosures refuse even when an exact quotient is small.
/// ´claim:scalar:literal-range-and-divisor-refusals´
/// ´test:integration:literal-range-and-divisor-refusals´
#[test]
fn literal_range_and_divisor_refusals() -> TestResult {
    let context = ArithmeticContext::new()?;
    for text in ["1e39", "1e39/1e39"] {
        assert!(matches!(
            NumericLiteral::new(text, LiteralLimits::default())?.decode::<f32>(&context),
            Err(ArithmeticFailure::LiteralRange)
        ));
    }
    assert!(matches!(
        NumericLiteral::new("1e309", LiteralLimits::default())?.decode::<f64>(&context),
        Err(ArithmeticFailure::LiteralRange)
    ));
    for text in ["1/0", "0/0", "1/1e-400"] {
        assert!(matches!(
            NumericLiteral::new(text, LiteralLimits::default())?.decode::<f64>(&context),
            Err(ArithmeticFailure::ZeroInDivisor)
        ));
    }
    Ok(())
}
