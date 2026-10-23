// SPDX-License-Identifier: AGPL-3.0-only
//! Checked chain-rule results retain the original map and the direct source theorem.

use super::SourceJacobian;
use crate::graph::Handle;
use crate::proof_model::VerifiedWitness;
use crate::readings::{ExactAffineMap, ReuseKey};

/// Ordered scalar reading variables defined over the same source coordinate vector.
///
/// Matrix and component adapters provide their exact scalar entry keys in logical
/// order. Repeated rows explicitly share source directions. Coefficients and
/// offsets may depend on source or fixed parameters, but never reading variables.
#[derive(Clone, Debug)]
pub struct JacobianReadings {
    /// Distinct declared Real scalar reading keys, in map output order.
    pub variables: Vec<String>,
    /// Checked exact affine syntax, including original coefficient domains.
    pub map: ExactAffineMap,
}

/// A directly proved source Jacobian and its independently checked chain-rule formula.
///
/// The equality witness proves the proposed composed matrix equal to the direct
/// derivative. No proposed outer derivative becomes a trusted premise. Original
/// reading-map domains survive in `map_representation`; keep the complete bundle.
#[derive(Clone, Debug)]
pub struct ComposedJacobian {
    /// Original expression before replacing reading variables.
    pub(super) original: Handle,
    /// Canonical original relation with its conditional reading domains.
    pub(super) original_key: ReuseKey,
    /// Ordered checked reading definition.
    pub(super) readings: JacobianReadings,
    /// Original homogeneous map equal to its explicit coefficients.
    pub(super) map_representation: VerifiedWitness,
    /// Proved Jacobian after simultaneous shared-source substitution.
    pub(super) direct: SourceJacobian,
    /// Exact chain-rule matrix product after evaluating outer partial proposals.
    pub(super) composed: Handle,
    /// Whole-family equality of the composed and directly checked Jacobians.
    pub(super) equality: VerifiedWitness,
}

impl ComposedJacobian {
    /// Original expression containing the declared scalar readings.
    #[must_use]
    pub const fn original(&self) -> &Handle {
        &self.original
    }

    /// Full canonical original-expression context and guard identity.
    #[must_use]
    pub const fn original_key(&self) -> &ReuseKey {
        &self.original_key
    }

    /// Ordered reading names and their retained original exact map.
    #[must_use]
    pub const fn readings(&self) -> &JacobianReadings {
        &self.readings
    }

    /// Original-map equality, including every original coefficient domain.
    #[must_use]
    pub const fn map_representation(&self) -> &VerifiedWitness {
        &self.map_representation
    }

    /// Direct source derivative, whose key is suitable for compatible numerical uses.
    #[must_use]
    pub const fn direct(&self) -> &SourceJacobian {
        &self.direct
    }

    /// Proposed chain-rule product, checked by the retained equality theorem.
    #[must_use]
    pub const fn composed(&self) -> &Handle {
        &self.composed
    }

    /// Ordinary portable equality between the composed and direct Jacobian matrices.
    #[must_use]
    pub const fn equality(&self) -> &VerifiedWitness {
        &self.equality
    }
}
