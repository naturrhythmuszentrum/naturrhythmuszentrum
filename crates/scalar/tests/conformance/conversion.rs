// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`exact_widening_preserves_coverage`] | scalar | Widening preserves exact centers and both endpoints, including every binary32 boundary. |
//! | [`outward_narrowing_rational_oracle`] | scalar | Subnormal loss, finite limits, signs and halfway values retain their exact original targets. |
//! | [`narrowing_halfway_ties`] | scalar | Nearest conversion obeys even-significand tie selection, including zero/subnormal ties. |
//! | [`narrowing_enclosure_and_range`] | scalar | Endpoint conversion is outward even when its rounded center and source center differ. |

//! Independent binary/rational precision-conversion oracles.

use crate::testing::{Binary, TestResult, contains, point};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, narrow, widen};
use num_traits::{Signed, Zero};

/// Widening preserves exact centers and both endpoints, including every binary32 boundary.
/// ´claim:scalar:exact-widening-preserves-coverage´
/// ´test:integration:exact-widening-preserves-coverage´
#[test]
fn exact_widening_preserves_coverage() -> TestResult {
    for bits in [0, 1, 0x007f_ffff, 0x0080_0000, 0x3dcc_cccd, 0x7f7f_ffff] {
        for value in [f32::from_bits(bits), -f32::from_bits(bits)] {
            let source = Finite::new(value)?;
            let converted = widen(source, point(value)?);
            assert_eq!(converted.center().get().rational(), value.rational());
            assert_eq!(converted.source_center(), source);
            assert_eq!(
                converted.enclosure().lower().get().rational(),
                value.rational()
            );
            assert_eq!(
                converted.enclosure().upper().get().rational(),
                value.rational()
            );
            assert!(converted.displacement().get().rational().is_zero());
        }
    }
    let original = Enclosure::new(-0.1_f32, 0.2)?;
    let wide = widen(Finite::ZERO, original);
    assert_eq!(
        wide.enclosure().lower().get().rational(),
        original.lower().get().rational()
    );
    assert_eq!(
        wide.enclosure().upper().get().rational(),
        original.upper().get().rational()
    );
    Ok(())
}

/// Check narrowing endpoints and additional displacement directly as rational inequalities.
fn narrowing(value: f64, context: &ArithmeticContext) -> TestResult {
    let converted = narrow(Finite::new(value)?, point(value)?, context)?;
    let exact = value.rational();
    contains(converted.enclosure(), &exact);
    let delta = (converted.center().get().rational() - &exact).abs();
    assert!(converted.displacement().get().rational() >= delta);
    assert!(converted.displacement().get() >= 0.0);
    let restored = widen(converted.center(), converted.enclosure());
    contains(restored.enclosure(), &exact);
    Ok(())
}

/// Subnormal loss, finite limits, signs and halfway values retain their exact original targets.
/// ´claim:scalar:outward-narrowing-rational-oracle´
/// ´test:integration:outward-narrowing-rational-oracle´
#[test]
fn outward_narrowing_rational_oracle() -> TestResult {
    let context = ArithmeticContext::new()?;
    let values = [
        0.0_f64,
        f64::from_bits(1),
        f64::from(f32::from_bits(1)),
        f64::from(f32::MIN_POSITIVE),
        0.1,
        1.0,
        f64::from_bits(0x3ff0_0000_1000_0000),
        f64::from_bits(0x3ff0_0000_3000_0000),
        f64::from_bits(0x3690_0000_0000_0000),
        f64::from_bits(0x36a8_0000_0000_0000),
        f64::from(f32::MAX).next_down(),
        f64::from(f32::MAX),
    ];
    for value in values {
        narrowing(value, &context)?;
        narrowing(-value, &context)?;
    }
    Ok(())
}

/// Nearest conversion obeys even-significand tie selection, including zero/subnormal ties.
/// ´claim:scalar:narrowing-halfway-ties´
/// ´test:integration:narrowing-halfway-ties´
#[test]
fn narrowing_halfway_ties() -> TestResult {
    let context = ArithmeticContext::new()?;
    for (source, expected) in [
        (0x3ff0_0000_1000_0000, 0x3f80_0000),
        (0x3ff0_0000_3000_0000, 0x3f80_0002),
        (0x3690_0000_0000_0000, 0),
        (0x36a8_0000_0000_0000, 2),
    ] {
        let value = f64::from_bits(source);
        let positive = narrow(Finite::new(value)?, point(value)?, &context)?;
        assert_eq!(positive.center().get().to_bits(), expected);
        let negative = narrow(Finite::new(-value)?, point(-value)?, &context)?;
        let negative_bits = if expected == 0 {
            0
        } else {
            expected | 0x8000_0000
        };
        assert_eq!(negative.center().get().to_bits(), negative_bits);
    }
    Ok(())
}

/// Endpoint conversion is outward even when its rounded center and source center differ.
/// ´claim:scalar:narrowing-enclosure-and-range´
/// ´test:integration:narrowing-enclosure-and-range´
#[test]
fn narrowing_enclosure_and_range() -> TestResult {
    let context = ArithmeticContext::new()?;
    let original = Enclosure::new(-0.1_f64, 0.2)?;
    let converted = narrow(Finite::ZERO, original, &context)?;
    contains(converted.enclosure(), &original.lower().get().rational());
    contains(converted.enclosure(), &original.upper().get().rational());
    let outside = f64::from(f32::MAX).next_up();
    for value in [outside, -outside, f64::MAX] {
        assert!(matches!(
            narrow(Finite::new(value)?, point(value)?, &context),
            Err(ArithmeticFailure::Overflow)
        ));
    }
    assert!(matches!(
        narrow(Finite::ZERO, Enclosure::new(0.0, outside)?, &context),
        Err(ArithmeticFailure::Overflow)
    ));
    let tiny = narrow(
        Finite::new(f64::from_bits(1))?,
        point(f64::from_bits(1))?,
        &context,
    )?;
    assert_eq!(tiny.center().get().to_bits(), 0);
    assert_eq!(tiny.displacement().get().to_bits(), 1);
    Ok(())
}
