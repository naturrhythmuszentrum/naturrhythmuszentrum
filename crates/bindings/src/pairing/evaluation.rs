// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical parameter error is distinct from uncertainty of either physical target.

use super::{PairingError, PairingWitness};
use crate::{Dimension, Measure, Role};

/// The semantic kind of a numerical evaluation enclosure on a named parameter relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationKind {
    /// A deterministic difference radius on the parameter itself.
    Radius,
}

/// An evaluation-radius descriptor targeting an entire companion or sensitivity parameter.
///
/// A covariance parameter keeps its product dimension and both ordered target
/// schemas. A relative parameter keeps its named reference without admitting a
/// nested relative scalar `Kind`. The numerical owner binds this descriptor to the
/// parameter region's identity and version, never to either original value region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationDescriptor {
    /// Checked parameter relation, including both sides and any named reference.
    target: PairingWitness,
    /// The admitted evaluation interpretation, separate from the parameter kind.
    kind: EvaluationKind,
}

impl EvaluationDescriptor {
    /// Derive an evaluation radius from an already checked parameter relation.
    #[must_use]
    pub fn new(target: &PairingWitness) -> Self {
        Self {
            target: target.clone(),
            kind: EvaluationKind::Radius,
        }
    }

    /// The parameter being evaluated, not its first or second physical target.
    #[must_use]
    pub const fn target(&self) -> &PairingWitness {
        &self.target
    }

    /// A deterministic evaluation-radius kind, separate from scalar uncertainty admission.
    #[must_use]
    pub const fn kind(&self) -> EvaluationKind {
        self.kind
    }

    /// The parameter's own dimension, including both covariance factors.
    #[must_use]
    pub const fn dimension(&self) -> &Dimension {
        self.target.dimension()
    }

    /// The parameter's difference measure; paired derivation has already removed origins.
    #[must_use]
    pub const fn measure(&self) -> &Measure {
        self.target.measure()
    }

    /// Evaluation error is always a difference, even for affine target coordinates.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.target.role()
    }

    /// Require the exact parameter pairing, including its mode and named reference.
    ///
    /// # Errors
    /// Refuses a different parameter even when dimensions and scales are equal.
    pub fn validate_target(&self, target: &PairingWitness) -> Result<(), PairingError> {
        if &self.target == target {
            Ok(())
        } else {
            Err(PairingError::EvaluationTargetMismatch)
        }
    }
}
