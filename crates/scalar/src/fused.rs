// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit fused evaluation of the multi-affine endpoint expression.
//!
//! # Fused and separate evaluation
//!
//! `Enclosure::mul_add` explicitly evaluates all eight endpoint triples with one fused rounding each. Separate `mul` and `add` calls retain two rounded operations. `sum` and `dot` accept plain slices and use a left fold in increasing index order starting at exact zero; dot multiplies before adding. Unequal dot lengths and overflowing intermediates refuse, even if reassociation could produce a finite answer.

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Scalar, primitive};

impl<P: Scalar> Enclosure<P> {
    /// Enclose `self * multiplier + addend` using one rounding per endpoint triple.
    ///
    /// The eight corner values enclose the multi-affine expression on the box.
    /// This is explicitly fused; `mul` followed by `add` performs two rounded
    /// operations and can refuse an intermediate that fused evaluation accepts.
    ///
    /// # Errors
    /// Refuses a nonfinite fused result or required outward neighbour.
    pub fn mul_add(
        self,
        multiplier: Self,
        addend: Self,
        _context: &ArithmeticContext,
    ) -> Result<Self, ArithmeticFailure> {
        let mut lower = P::max_finite();
        let mut upper = -P::max_finite();
        for left in [self.lower(), self.upper()] {
            for right in [multiplier.lower(), multiplier.upper()] {
                for offset in [addend.lower(), addend.upper()] {
                    let value = primitive::rounded(left.get().mul_add(right.get(), offset.get()))?;
                    if value.lower().get() < lower {
                        lower = value.lower().get();
                    }
                    if value.upper().get() > upper {
                        upper = value.upper().get();
                    }
                }
            }
        }
        Self::new(lower, upper)
    }
}
