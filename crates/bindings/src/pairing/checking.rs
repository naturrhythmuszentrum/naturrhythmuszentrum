// SPDX-License-Identifier: AGPL-3.0-only
//! Concrete scalar entries must satisfy kinds as well as formal pairing dimensions.

use super::{Entry, NamedReference, PairingError, PairingSide, PairingWitness};
use crate::{BinaryOp, Conversion, Descriptor, Registry, Role, UnitError};

impl PairingWitness {
    /// Check an actual scalar entry and return its exact map into the pairing's coordinates.
    ///
    /// An uncertainty must have the derived paired uncertainty kind. A sensitivity
    /// coefficient must map its input difference to its output difference through
    /// an existing registry multiplication rule. Dimension equality alone is insufficient.
    ///
    /// # Errors
    /// Refuses wrong entry kinds, roles or dimensions, missing semantic rules and exact overflow.
    pub fn check_entry(
        &self,
        registry: &Registry,
        entry: &Descriptor,
    ) -> Result<Conversion, PairingError> {
        registry.validate(entry)?;
        registry.validate(&self.target)?;
        match &self.entry {
            Entry::Uncertainty { descriptor, .. } => Ok(entry.conversion_to(descriptor)?),
            Entry::Sensitivity { dimension, measure } => {
                if entry.dimension() != dimension {
                    return Err(UnitError::DimensionMismatch.into());
                }
                let input = self
                    .other
                    .as_ref()
                    .ok_or(PairingError::ExpectedSensitivity)?;
                let output = registry.binary(BinaryOp::Multiply, entry, &input.difference())?;
                output.compatible_kind(&self.target.difference())?;
                Ok(entry.measure().conversion_to(measure, Role::Difference)?)
            }
        }
    }

    /// Require the exact original target schemas in their captured order.
    ///
    /// This checks descriptor identities only. Concrete value IDs, versions, selections
    /// and explicit target conversions remain the matrix binding's responsibility.
    ///
    /// # Errors
    /// Identifies the first mismatched side, including missing or extra second targets.
    pub fn validate_targets(
        &self,
        target: &Descriptor,
        other: Option<&Descriptor>,
    ) -> Result<(), PairingError> {
        if &self.target != target {
            Err(PairingError::TargetMismatch {
                side: PairingSide::First,
            })
        } else if self.other.as_ref() != other {
            Err(PairingError::TargetMismatch {
                side: PairingSide::Second,
            })
        } else {
            Ok(())
        }
    }

    /// Require the exact named reference when binding a relative companion schema.
    ///
    /// # Errors
    /// Refuses a missing, extra or differently named/reference-coordinate declaration.
    pub fn validate_reference(
        &self,
        reference: Option<&NamedReference>,
    ) -> Result<(), PairingError> {
        if self.reference.as_ref() == reference {
            Ok(())
        } else {
            Err(PairingError::ReferenceMismatch)
        }
    }
}
