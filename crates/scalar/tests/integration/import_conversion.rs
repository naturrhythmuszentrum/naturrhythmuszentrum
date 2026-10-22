// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`literal_precision_change_lifecycle`] | scalar | Import error and new narrowing displacement cover the changed representation together. |

//! Numeric import and explicit precision changes retain both error contributions.

use crate::testing::{Binary, TestResult, contains};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, narrow, widen};
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::Signed;

/// Import error and new narrowing displacement cover the changed representation together.
/// ´claim:scalar:literal-precision-change-lifecycle´
/// ´test:integration:literal-precision-change-lifecycle´
#[test]
fn literal_precision_change_lifecycle() -> TestResult {
    let context = ArithmeticContext::new()?;
    let imported = NumericLiteral::new("0.1", LiteralLimits::default())?.decode::<f64>(&context)?;
    let converted = narrow(imported.center(), imported.enclosure(), &context)?;
    let restored = widen(converted.center(), converted.enclosure());
    let exact = Ratio::new(BigInt::from(1), BigInt::from(10));
    contains(restored.enclosure(), &exact);
    let error = (converted.center().get().rational() - &exact).abs();
    let total =
        imported.displacement().get().rational() + converted.displacement().get().rational();
    assert!(total >= error);
    assert_eq!(restored.displacement().get().to_bits(), 0);
    let single_import =
        NumericLiteral::new("0.1", LiteralLimits::default())?.decode::<f32>(&context)?;
    let widened = widen(single_import.center(), single_import.enclosure());
    contains(widened.enclosure(), &exact);
    assert!(
        single_import.displacement().get().rational()
            >= (widened.center().get().rational() - exact).abs()
    );
    Ok(())
}
