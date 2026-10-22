// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`dot_to_norm_lifecycle`] | scalar | A complete small contraction/norm lifecycle carries its independent exact target. |

//! A contraction and norm bound composed through the public scalar API.

use crate::testing::{Binary, TestResult, contains};
use nrz_scalar::{ArithmeticContext, dot, norm_upper};
use num_traits::Signed;

/// Dot enclosures can be summarized conservatively by a norm of their magnitudes.
fn lifecycle<P: Binary>(left: &[P], right: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let exact = left
        .iter()
        .zip(right)
        .map(|(&first, &second)| first.rational() * second.rational())
        .sum();
    let result = dot(left, right, &context)?;
    contains(result, &exact);
    let magnitude = result.abs().upper().get();
    let bound = norm_upper(std::slice::from_ref(&magnitude), &context)?;
    assert!(bound.get().rational() >= exact.abs());
    Ok(())
}

/// A complete small contraction/norm lifecycle carries its independent exact target.
/// ´claim:scalar:dot-to-norm-lifecycle´
/// ´test:integration:dot-to-norm-lifecycle´
#[test]
fn dot_to_norm_lifecycle() -> TestResult {
    lifecycle(&[1.0_f32, 2.0, -3.0], &[2.0, 3.0, 1.0])?;
    lifecycle(&[1.0_f64, 2.0, -3.0], &[2.0, 3.0, 1.0])
}
