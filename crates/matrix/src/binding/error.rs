// SPDX-License-Identifier: AGPL-3.0-only
//! Transaction admission failures retain their underlying storage and shape causes.

/// A refused operation contract, binding, evaluation or publication.
#[derive(Clone, Debug, thiserror::Error)]
pub enum OperationError {
    /// Private numerical allocation did not satisfy its prepared exact inventory.
    #[error("private allocation: {0}")]
    PrivateAllocation(#[from] crate::PrivateAllocationError),
    /// Exact coefficient recipe, reference or correspondence admission refused.
    #[error("reading recipe: {0}")]
    Reading(#[from] nrz_bindings::readings::ReadingError),
    /// Cumulative exact construction or coefficient export resources refused.
    #[error("recipe algebra: {0}")]
    Exact(#[from] nrz_bindings::readings::algebra::Fault),
    /// Square solve admission, candidate generation or independent certification failed.
    #[error("solve: {0}")]
    Solve(#[from] crate::SolveError),
    /// Statistical source, parameter, domain or propagation refused.
    #[error("statistics: {0}")]
    Statistics(#[from] crate::StatisticsError),
    /// Deterministic kernel arithmetic, domain or coverage refused.
    #[error("kernel: {0}")]
    Kernel(#[from] crate::KernelError),
    /// Checked numerical conversion refused before publication.
    #[error("conversion: {0}")]
    Conversion(#[from] crate::conversion::ConversionError),
    /// Heap resolution, capacity or payload validation failed.
    #[error("heap transaction: {0}")]
    Heap(#[from] crate::HeapError),
    /// Physical shape or reference checking failed.
    #[error("operation shape: {0}")]
    Shape(#[from] crate::ShapeError),
    /// A copy contract needs concrete homogeneous difference entry descriptors.
    #[error("copy requires a homogeneous difference shape")]
    UnsupportedShape,
    /// A concrete operand differs from its checked slot contract.
    #[error("input slot {slot} does not match its checked schema")]
    InputMismatch {
        /// Operation-ordered input position.
        slot: usize,
    },
    /// A declared structural zero disagrees with the center or whole-family bound.
    #[error("structural zero at input {slot}, entry {entry} is not established")]
    StructureMismatch {
        /// Operation-ordered input position.
        slot: usize,
        /// Logical column-ordered entry.
        entry: usize,
    },
    /// The input list does not match the closed recipe.
    #[error("operation input count mismatch")]
    InputCount,
    /// Explicit execution work or scratch capacity was exhausted.
    #[error("execution resource limit exhausted")]
    ResourceLimit,
    /// Input evidence belongs to another binding's execution.
    #[error("normalized inputs belong to another execution")]
    ForeignInputs,
    /// A candidate belongs to another binding or its token was not issued here.
    #[error("candidate belongs to another execution or its token was consumed")]
    ForeignCandidate,
    /// This binding has already issued its one candidate token.
    #[error("execution has already issued a candidate")]
    AlreadyEvaluated,
    /// A private candidate does not satisfy the reserved complete output contract.
    #[error("candidate output or companion contract is incomplete")]
    CandidateContract,
    /// The immutable provenance graph would exceed its explicit capacity or work policy.
    #[error("provenance resource limit exhausted")]
    ProvenanceLimit,
}

impl From<nrz_bindings::MapError> for OperationError {
    fn from(cause: nrz_bindings::MapError) -> Self {
        Self::Conversion(cause.into())
    }
}
