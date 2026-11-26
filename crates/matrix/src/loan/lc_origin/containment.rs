// SPDX-License-Identifier: AGPL-3.0-only
//! One current original reading supplies a numerical containment comparison only.
//! A total outer cover can be checked even when its numerical companion has local scope.

use crate::LcInputReading;
use nrz_bindings::{
    Budget, LcArithmeticMode, LcContainmentError, LcStratum, contains_binary32, contains_binary64,
    decode_binary32, decode_binary64,
};

impl LcInputReading<f32> {
    /// Compare this authenticated original input's total box to a binary32 stratum.
    /// The caller retains the current source/selection closure; this transient result
    /// is no persistent application or proof of feasibility.
    /// # Errors
    /// Refuses malformed companions, unsupported modes or exact failure.
    pub fn check_containment32(
        &self,
        endpoints: [f32; 2],
        stratum: &LcStratum<f32>,
        mode: LcArithmeticMode,
        budget: &mut Budget,
    ) -> Result<bool, LcContainmentError> {
        let [center, total, numerical] = self.values();
        if !numerical.is_finite() || numerical < 0.0 || !total.is_finite() || total < numerical {
            return Err(LcContainmentError::InvalidInput);
        }
        let c = decode_binary32(center.to_bits(), budget)?;
        let t = decode_binary32(total.to_bits(), budget)?;
        let low = c.sub(&t, budget)?;
        let high = c.add(&t, budget)?;
        contains_binary32(endpoints, [&low, &high], stratum, mode, budget)
    }
}

impl LcInputReading<f64> {
    /// Compare this authenticated original input's total box to a binary64 stratum.
    /// The numerical companion is checked but is already included once in total.
    /// # Errors
    /// Refuses malformed companions, unsupported modes or exact failure.
    pub fn check_containment64(
        &self,
        endpoints: [f64; 2],
        stratum: &LcStratum<f64>,
        mode: LcArithmeticMode,
        budget: &mut Budget,
    ) -> Result<bool, LcContainmentError> {
        let [center, total, numerical] = self.values();
        if !numerical.is_finite() || numerical < 0.0 || !total.is_finite() || total < numerical {
            return Err(LcContainmentError::InvalidInput);
        }
        let c = decode_binary64(center.to_bits(), budget)?;
        let t = decode_binary64(total.to_bits(), budget)?;
        let low = c.sub(&t, budget)?;
        let high = c.add(&t, budget)?;
        contains_binary64(endpoints, [&low, &high], stratum, mode, budget)
    }
}
