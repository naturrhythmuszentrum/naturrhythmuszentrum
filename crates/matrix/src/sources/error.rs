// SPDX-License-Identifier: AGPL-3.0-only
//! Refusals distinguish insufficient evidence from mathematical impossibility.

/// A source declaration, covariance admission or companion relation was refused.
#[derive(Clone, Debug, thiserror::Error)]
pub enum JointError {
    /// Storage resolution or recoverable allocation failed.
    #[error("joint storage: {0}")]
    Heap(#[from] crate::HeapError),
    /// Outward arithmetic could not produce finite evidence.
    #[error("joint arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// Bindings refused a companion pairing.
    #[error("joint pairing: {0}")]
    Pairing(#[from] nrz_bindings::PairingError),
    /// Explicit storage or work policy was exhausted.
    #[error("joint model resource limit exhausted")]
    ResourceLimit,
    /// Extents or complete block-table lengths disagree.
    #[error("joint model length mismatch")]
    LengthMismatch,
    /// A source must be a coherent continuous difference with a positive revision.
    #[error("invalid source declaration")]
    InvalidSource,
    /// One immutable source identity has conflicting definitions or joint relations.
    #[error("conflicting definitions under one source identity")]
    SourceConflict,
    /// A factor weight is negative or nonfinite.
    #[error("invalid nonnegative factor weight")]
    InvalidWeight,
    /// Symmetric entries or block declarations disagree exactly.
    #[error("covariance is not exactly symmetric")]
    Asymmetric,
    /// The sufficient test did not establish positive semidefiniteness.
    #[error("cannot validate covariance")]
    CannotValidateCovariance,
    /// A contraction needs an explicitly unknown block.
    #[error("requested contraction has an unknown cross block")]
    MissingCrossBlock,
    /// A zero-covariance declaration is empty or unsupported by the witness.
    #[error("invalid zero-covariance declaration")]
    InvalidZero,
    /// A selected target, order, schema or current version differs.
    #[error("companion target or source order mismatch")]
    WrongTarget,
    /// This subset cannot preserve the requested companion interpretation.
    #[error("unsupported joint companion mode")]
    UnsupportedMode,
}
