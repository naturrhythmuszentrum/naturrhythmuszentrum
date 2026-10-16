// SPDX-License-Identifier: AGPL-3.0-only
//! Public-only rational endpoint preparation for the nonlinear root conformance oracle.

use crate::conversion_testing::TestResult;
use crate::kernel_oracle::{Rational, gcd};

impl Rational {
    /// Move a dyadic endpoint inward to a bounded denominator before squaring it.
    pub(crate) fn dyadic_inward(self, ceil: bool) -> TestResult<Self> {
        let scale = 1_i128 << 60;
        let common = gcd(scale, self.denominator);
        let numerator = self
            .numerator
            .checked_mul(scale / common)
            .ok_or("oracle overflow")?;
        let denominator = self.denominator / common;
        let floor = numerator.div_euclid(denominator);
        let integer = if ceil && numerator.rem_euclid(denominator) != 0 {
            floor.checked_add(1).ok_or("oracle overflow")?
        } else {
            floor
        };
        Self::new(integer, scale)
    }
}
