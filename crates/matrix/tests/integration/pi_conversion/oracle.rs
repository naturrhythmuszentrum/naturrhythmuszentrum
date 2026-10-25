// SPDX-License-Identifier: AGPL-3.0-only
//! Independent integer atan identity and dyadic projections use no production pi constants.

use crate::{conversion_testing::TestResult, kernel_oracle::Rational};

/// Bound atan(1/q) by term floors and the signed alternating-series remainder.
fn arctangent(q: i128) -> [i128; 2] {
    let mut power = (1_i128 << 80) / q;
    let mut bounds = [0, 0];
    for index in 0..64 {
        let term = power / (2 * index + 1);
        if index % 2 == 0 {
            bounds[0] += term;
            bounds[1] += term + 1;
        } else {
            bounds[0] -= term + 1;
            bounds[1] -= term;
        }
        power /= q * q;
    }
    bounds[1] += power / 129 + 1;
    bounds
}

/// Project pi = 4(atan(1/2) + atan(1/3)) outward onto denominator two to the 56.
fn circle() -> [i128; 2] {
    let first = arctangent(2);
    let second = arctangent(3);
    let unit = 1_i128 << 24;
    [
        4 * (first[0] + second[0]) / unit,
        (4 * (first[1] + second[1]) + unit - 1) / unit,
    ]
}

/// Independently enclose positive n*pi/d or n/(d*pi) at a chosen dyadic resolution.
pub(crate) fn factor(n: i128, d: i128, inverse: bool, bits: u32) -> TestResult<[Rational; 2]> {
    let bounds = circle();
    let scale = 1_i128 << bits;
    let original = 1_i128 << 56;
    let (low, high) = if inverse {
        let top = n * original * scale;
        (
            top / (d * bounds[1]),
            (top + d * bounds[0] - 1) / (d * bounds[0]),
        )
    } else {
        let bottom = d * original;
        (
            n * bounds[0] * scale / bottom,
            (n * bounds[1] * scale + bottom - 1) / bottom,
        )
    };
    Ok([Rational::new(low, scale)?, Rational::new(high, scale)?])
}

/// Both independently projected endpoints must lie in the returned numerical error ball.
pub(crate) fn covers(bounds: [Rational; 2], center: f64, error: f64) -> TestResult {
    for endpoint in bounds {
        assert!(
            endpoint.covered(center, error)?,
            "independent endpoint is outside the error ball"
        );
    }
    Ok(())
}
