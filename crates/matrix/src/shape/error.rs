// SPDX-License-Identifier: AGPL-3.0-only
//! Shape refusals retain scalar causes and logical locations.

/// A descriptor or schema check failed before any numerical execution.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ShapeError {
    /// Checked metadata arithmetic overflowed.
    #[error("shape extent arithmetic overflowed")]
    ExtentOverflow,
    /// Explicit shape work or allocation capacity was exhausted.
    #[error("shape work or capacity limit exhausted")]
    Limit,
    /// An axis or reference label is empty.
    #[error("coordinate label must be nonempty")]
    EmptyName,
    /// Ordered coordinate identities or their interpretations disagree.
    #[error("ordered axes do not agree")]
    AxisMismatch,
    /// A linear contract was given a point or unsupported formal axis.
    #[error("linear assembly requires ordinary difference axes")]
    LinearAxis,
    /// Partitions are malformed, overlapping or incomplete.
    #[error("invalid partition or incomplete block coverage")]
    Partition,
    /// A block payload or explicit map has the wrong length.
    #[error("logical entry count does not match the contract")]
    LengthMismatch,
    /// A logical index is out of range.
    #[error("logical entry index is out of range")]
    EntryIndex,
    /// A pairing refuses this column-ordered original entry.
    #[error("entry {index} does not map its column to its row: {cause}")]
    EntryMeaning {
        /// Logical index in column order.
        index: usize,
        /// Preserved units refusal.
        cause: nrz_units::PairingError,
    },
    /// The contract uses another frozen semantic state.
    #[error("shape declaration snapshots differ")]
    SnapshotMismatch,
    /// A nonempty square operator and positive RHS width are required.
    #[error("solve requires positive square order and at least one right-hand side")]
    SolveExtent,
    /// A scalar operation could not be admitted.
    #[error("units descriptor check failed: {0}")]
    Units(#[from] nrz_units::UnitError),
    /// Formal pairing derivation failed.
    #[error("units pairing check failed: {0}")]
    Pairing(#[from] nrz_units::PairingError),
    /// Original declaration evidence is unavailable or invalid.
    #[error("original declaration check failed: {0}")]
    Original(Box<nrz_units::Diagnostic>),
    /// A requested operation needs a units surface not supplied by this subset.
    #[error("unsupported shape contract: {0}")]
    Unsupported(&'static str),
}

impl From<nrz_units::Diagnostic> for ShapeError {
    fn from(value: nrz_units::Diagnostic) -> Self {
        Self::Original(Box::new(value))
    }
}

impl From<crate::HeapError> for ShapeError {
    fn from(value: crate::HeapError) -> Self {
        match value {
            crate::HeapError::ExtentOverflow => Self::ExtentOverflow,
            _ => Self::EntryIndex,
        }
    }
}
