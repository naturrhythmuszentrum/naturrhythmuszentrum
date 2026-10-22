// SPDX-License-Identifier: AGPL-3.0-only
//! Layout-free companion and formal sensitivity relations, never numerical witnesses.

mod checking;
mod contraction;
mod derive;
mod error;
mod evaluation;
mod reference;

use crate::{Descriptor, Dimension, Measure, Role, UncertaintyMode};

pub use error::{PairingError, PairingSide};
pub use evaluation::{EvaluationDescriptor, EvaluationKind};
pub use reference::NamedReference;

/// The interpretation of a companion entry, distinct from its dimensions or extents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingKind {
    /// An uncertainty mode with explicit ordered target descriptors.
    Uncertainty(UncertaintyMode),
    /// A formal output-difference over input-difference entry.
    Sensitivity,
}

/// Private entry representation keeps formal quotients out of the scalar kind registry.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Entry {
    /// A scalar uncertainty descriptor constructed by the existing uncertainty rules.
    Uncertainty {
        /// The admitted interpretation, retained without reverse kind inference.
        mode: UncertaintyMode,
        /// Checked dimension, paired kind and difference measure.
        descriptor: Descriptor,
    },
    /// A relational quotient, which does not invent a named scalar kind.
    Sensitivity {
        /// Output dimension minus input dimension.
        dimension: Dimension,
        /// Output difference scale divided by input difference scale.
        measure: Measure,
    },
}

/// Checked scalar pairing semantics independent of layouts, heap targets and numerical values.
///
/// The first target is the uncertainty value or sensitivity output. The second is
/// the covariance's other target or sensitivity input. Variance pairs its first
/// target with itself. Equal schemas do not establish equal value identities or
/// independence; matrix must bind both sides to concrete entries and versions.
/// This witness alone proves neither a derivative formula nor covariance validity.
#[doc = include_str!("../../tests/conformance/pairing.inc")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingWitness {
    /// Validated entry representation, with no mutable public escape.
    entry: Entry,
    /// Original first target, including its affine origin and role.
    target: Descriptor,
    /// Original second target, absent for one-target uncertainty modes.
    other: Option<Descriptor>,
    /// A named nonzero difference reference, present exactly for relative modes.
    reference: Option<NamedReference>,
}

impl PairingWitness {
    /// Formal interpretation; it is never inferred from equal dimensions.
    #[must_use]
    pub const fn kind(&self) -> PairingKind {
        match &self.entry {
            Entry::Uncertainty { mode, .. } => PairingKind::Uncertainty(*mode),
            Entry::Sensitivity { .. } => PairingKind::Sensitivity,
        }
    }

    /// First target schema; covariance order and affine origins remain explicit.
    #[must_use]
    pub const fn target(&self) -> &Descriptor {
        &self.target
    }

    /// Second covariance target or sensitivity input schema.
    #[must_use]
    pub const fn other(&self) -> Option<&Descriptor> {
        self.other.as_ref()
    }

    /// Named reference retained by a relative companion.
    #[must_use]
    pub const fn reference(&self) -> Option<&NamedReference> {
        self.reference.as_ref()
    }

    /// Derived entry dimension, including products or formal quotients.
    #[must_use]
    pub const fn dimension(&self) -> &Dimension {
        match &self.entry {
            Entry::Uncertainty { descriptor, .. } => descriptor.dimension(),
            Entry::Sensitivity { dimension, .. } => dimension,
        }
    }

    /// Derived difference measure, always with zero affine offset.
    #[must_use]
    pub const fn measure(&self) -> &Measure {
        match &self.entry {
            Entry::Uncertainty { descriptor, .. } => descriptor.measure(),
            Entry::Sensitivity { measure, .. } => measure,
        }
    }

    /// Companion and sensitivity coordinates are linear differences.
    #[must_use]
    pub const fn role(&self) -> Role {
        match &self.entry {
            Entry::Uncertainty { descriptor, .. } => descriptor.role(),
            Entry::Sensitivity { .. } => Role::Difference,
        }
    }

    /// Scalar uncertainty descriptor; a formal sensitivity has no invented scalar kind.
    #[must_use]
    pub const fn uncertainty_descriptor(&self) -> Option<&Descriptor> {
        match &self.entry {
            Entry::Uncertainty { descriptor, .. } => Some(descriptor),
            Entry::Sensitivity { .. } => None,
        }
    }
}
