// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor contractions preserve both statistical sides and never assert numerical propagation.

use super::{PairingError, PairingKind, PairingSide, PairingWitness};
use crate::{Descriptor, UncertaintyMode};

impl PairingWitness {
    /// Derive the paired output schema of one term in `J_left U J_right^T`.
    ///
    /// Both maps must be sensitivities and their input schemas must exactly match
    /// the ordered covariance targets. A variance repeats its primary target. Matrix
    /// owns the contraction indices, derivative witnesses, complete source model and
    /// numerical evaluation; this method establishes only the entry descriptor law.
    ///
    /// # Errors
    /// Refuses wrong interpretations or either mismatched target side; exact descriptor
    /// arithmetic can also refuse before any numerical operation exists.
    pub fn contract_covariance(
        &self,
        covariance: &Self,
        right: &Self,
    ) -> Result<Self, PairingError> {
        let left_input = self.sensitivity_input()?;
        let right_input = right.sensitivity_input()?;
        let (first, second) = covariance.covariance_targets()?;
        match_side(left_input, first, PairingSide::First)?;
        match_side(right_input, second, PairingSide::Second)?;
        Self::uncertainty(
            UncertaintyMode::Covariance,
            &self.target,
            Some(&right.target),
            None,
        )
    }

    /// Derive the output radius schema of one absolute sensitivity-times-radius term.
    ///
    /// Numerical absolute values, sums, remainders and evaluation error belong to
    /// the matrix kernel; a standard uncertainty or relative radius cannot stand in
    /// for an absolute deterministic bound.
    ///
    /// # Errors
    /// Refuses a non-sensitivity, non-radius, mismatched target or exact overflow.
    pub fn contract_radius(&self, radius: &Self) -> Result<Self, PairingError> {
        let input = self.sensitivity_input()?;
        if radius.kind() != PairingKind::Uncertainty(UncertaintyMode::Radius) {
            return Err(PairingError::ExpectedRadius);
        }
        match_side(input, &radius.target, PairingSide::First)?;
        Self::uncertainty(UncertaintyMode::Radius, &self.target, None, None)
    }

    /// Private extraction requires the formal quotient interpretation before reading a side.
    fn sensitivity_input(&self) -> Result<&Descriptor, PairingError> {
        if self.kind() != PairingKind::Sensitivity {
            return Err(PairingError::ExpectedSensitivity);
        }
        self.other.as_ref().ok_or(PairingError::ExpectedSensitivity)
    }

    /// Variance is the diagonal pairing; other uncertainty modes are not covariances.
    fn covariance_targets(&self) -> Result<(&Descriptor, &Descriptor), PairingError> {
        match self.kind() {
            PairingKind::Uncertainty(UncertaintyMode::Variance) => Ok((&self.target, &self.target)),
            PairingKind::Uncertainty(UncertaintyMode::Covariance) => Ok((
                &self.target,
                self.other
                    .as_ref()
                    .ok_or(PairingError::ExpectedCovariance)?,
            )),
            _ => Err(PairingError::ExpectedCovariance),
        }
    }
}

/// A coefficient coordinate denominator must refer to this exact target schema.
fn match_side(
    input: &Descriptor,
    target: &Descriptor,
    side: PairingSide,
) -> Result<(), PairingError> {
    if input == target {
        Ok(())
    } else {
        Err(PairingError::TargetMismatch { side })
    }
}
