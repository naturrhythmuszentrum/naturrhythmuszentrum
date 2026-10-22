// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor-level refusals, independent of expression locations.

use nrz_cas::Fault;

/// A mechanical descriptor check failed.
///
/// TODO ´todo:code:represent-unsupported-shape-kinds-and-pairing´: represent unsupported shape kinds and pairing relations distinctly from scalar dimension mismatch under rule:nrz:shape-entry-contract and rule:nrz:shape-transpose-and-duality.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UnitError {
    /// Recursive kind relationships exceeded the supported structural depth.
    #[error("kind relationship depth exceeds 128")]
    KindDepth,
    /// Exact descriptor arithmetic exceeded its finite representation.
    #[error(transparent)]
    Arithmetic(#[from] Fault),
    /// A dimension exponent exceeds the signed numerator or unsigned denominator range.
    #[error("dimension exponent is outside the signed 64-bit component range")]
    ExponentRange,
    /// The requested dimension is outside the selected exponent policy.
    #[error("fractional dimension is not admitted by the exponent policy")]
    NonIntegralDimension,
    /// Unchecked textual scale definitions provide no CAS expression or positivity evidence.
    #[error("unsupported measure scale: {0}")]
    UnsupportedMeasureScale(String),
    /// The numerical decoder only accepts rational constants, so an enclosure is still required.
    #[error("unsupported exact constant export: non-rational enclosure required")]
    UnsupportedExactConstantExport,
    /// A measure's exact scale must have a checked positive finite witness.
    #[error("measure scale must be strictly positive")]
    InvalidScale,
    /// A measure needs a nonempty reporting name.
    #[error("measure name must not be empty")]
    InvalidMeasureName,
    /// A difference has no affine coordinate offset.
    #[error("an affine offset cannot belong to a difference")]
    OffsetOnDifference,
    /// The operation's normalized dimensions differ.
    #[error("incompatible dimensions")]
    DimensionMismatch,
    /// Equal dimensions do not imply compatible kinds.
    #[error("incompatible semantic kinds")]
    KindMismatch,
    /// An exact descriptor relation is being applied to a different captured source schema.
    #[error("descriptor relation source differs from its checked schema")]
    RelationSource,
    /// The affine roles do not admit this operation.
    #[error("invalid affine-role operation")]
    AffineRole,
    /// No explicit semantic composition rule admits this operation.
    #[error("unsupported semantic-kind operation")]
    UnsupportedKindRule,
    /// A registry key already has a rule.
    #[error("duplicate semantic-kind rule")]
    DuplicateRule,
    /// A kind name has conflicting declarations.
    #[error("conflicting kind declaration: {}", .0.as_str())]
    ConflictingKind(String),
    /// A count domain is inconsistent or its closure cannot be established.
    #[error("count-domain condition is invalid or unproved")]
    CountDomain,
    /// A count literal is fractional or outside its declared domain.
    #[error("count literal is not an integer in the declared domain")]
    CountValue,
    /// Count coordinates must preserve their declared whole-number meaning.
    #[error("count measure must have scale one and offset zero")]
    CountMeasure,
    /// A covariance needs exactly two value descriptors.
    #[error("uncertainty relationship has the wrong number of value descriptors")]
    UncertaintyRelationship,
    /// An expression belongs to different value or reference symbols than its companion.
    #[error("uncertainty value or reference identity differs from its declaration")]
    UncertaintyIdentity,
    /// Relative uncertainty requires an explicit compatible nonzero difference reference.
    #[error("relative uncertainty requires a compatible nonzero difference reference")]
    RelativeReference,
}
