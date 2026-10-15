// SPDX-License-Identifier: AGPL-3.0-only
//! Typed import and resolution refusals.

/// A refused heap transaction or access; no partial bundle is published.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HeapError {
    /// The handle belongs to a different heap.
    #[error("handle belongs to another heap")]
    ForeignHandle,
    /// The descriptor or arena occupation no longer exists.
    #[error("handle or arena occupation is stale")]
    StaleHandle,
    /// A cached physical region predates the current storage relocation epoch.
    #[error("physical layout epoch is stale")]
    StaleLayout,
    /// An internal or erased category does not match the requested access.
    #[error("descriptor category does not match access")]
    CategoryMismatch,
    /// A descriptor or allocation has the wrong scalar format.
    #[error("scalar precision does not match allocation")]
    PrecisionMismatch,
    /// Extents, offsets or allocation sizes cannot be represented.
    #[error("region extent or address arithmetic overflowed")]
    ExtentOverflow,
    /// A payload length differs from the declared extent.
    #[error("payload length differs from the region extent")]
    LengthMismatch,
    /// A requested index or region lies outside its allocation.
    #[error("region or entry is outside its allocation")]
    InvalidLayout,
    /// Explicit layout enumeration or packing work was exhausted.
    #[error("layout work limit exhausted")]
    WorkLimit,
    /// An output repeats an address or intersects another output destination.
    #[error("output destinations are not injective and mutually disjoint")]
    OutputOverlap,
    /// A partial selected replacement cannot supply bounds for unselected entries.
    #[error("declared bounds are missing for unselected base entries")]
    MissingCoverage,
    /// A center is not finite.
    #[error("center at logical index {index} is not finite")]
    NonFinite {
        /// Column-ordered logical entry index.
        index: usize,
    },
    /// A declared radius is negative or not finite.
    #[error("bound at logical index {index} is negative or not finite")]
    InvalidBound {
        /// Column-ordered logical entry index.
        index: usize,
    },
    /// Explicit capacity or a fallible allocation refused the transaction.
    #[error("heap capacity exhausted")]
    Capacity,
    /// A process-local identity or occupation counter cannot advance.
    #[error("identity counter exhausted")]
    CounterExhausted,
    /// A companion does not cover the current target occupation and version.
    #[error("companion target evidence is stale")]
    StaleEvidence,
    /// The caller's expected content version differs from the current version.
    #[error("expected value version {expected:?}, found {actual:?}")]
    VersionMismatch {
        /// Version captured by the caller.
        expected: super::ValueVersion,
        /// Current version of the surviving occupation.
        actual: super::ValueVersion,
    },
    /// Floating arenas cannot hold discrete counts.
    #[error("discrete counts require an integer arena")]
    CountStorage,
    /// Presentation coordinates need a conversion kernel before import.
    #[error("import requires coherent coordinates")]
    NonCoherentMeasure,
    /// Units could not derive the requested radius descriptor.
    #[error("units descriptor check failed: {0}")]
    Units(#[from] nrz_units::UnitError),
}
