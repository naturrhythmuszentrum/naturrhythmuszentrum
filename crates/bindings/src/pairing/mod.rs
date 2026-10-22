// SPDX-License-Identifier: AGPL-3.0-only
//! Layout-free companion and formal sensitivity relations, never numerical witnesses.
//!
//! # Companion, sensitivity and contraction contracts
//!
//! `PairingWitness::uncertainty` derives componentwise radius and standard schemas, variance schemas and ordered covariance/cross-covariance entry schemas through the existing scalar uncertainty rules. Covariance dimensions add both target dimensions, its measure multiplies both difference scales, and its kind retains both target kinds. All companion offsets are zero. `NamedReference` requires a nonempty declaration key, a difference descriptor and an explicit nonzero assumption. Relative radius and relative standard schemas retain that reference; a missing, mismatched or extra reference refuses. `validate_targets` and `validate_reference` check exact captured schemas and reference declarations. They do not resolve concrete value identities, indices, extents, sources or versions (see `docs/design/matrix-core-design.md`).
//!
//! `PairingWitness::sensitivity` derives an output-over-input dimension and difference-scale quotient, retaining both original target schemas, including point roles and origins. It creates a formal relation rather than a scalar named kind or a registry multiplication rule. `check_entry` checks a supplied scalar coefficient through the existing registry before returning its exact coordinate map; uncertainty entries must have the derived uncertainty kind. Discrete count targets and unnamed relative sensitivity coordinates refuse; a separately declared continuous quantity may have its own sensitivity. A schema alone does not establish a derivative formula. Native symbolic derivative witnesses are supplied by the checked sensitivity recipes documented in `src/sensitivity/` (see `docs/design/matrix-core-design.md`).
//!
//! `contract_covariance` checks both input sides of one descriptor term in `J_left U J_right^T`, accepting covariance or a variance with its repeated target. It derives the paired output covariance schema without inferring independence or performing a numerical contraction. `contract_radius` accepts only an absolute deterministic radius. Wrong modes and target-side mismatches refuse even for equal dimensions; changed measures, affine origins or roles require explicitly rederived pairings. Matrix owns the ordered maps, actual derivative evidence, coverage, source model and numerical propagation.

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
