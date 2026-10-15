// SPDX-License-Identifier: AGPL-3.0-only
//! Transaction admission failures retain their underlying storage and shape causes.

/// A refused operation contract, binding, evaluation or publication.
#[derive(Clone, Debug, thiserror::Error)]
pub enum OperationError {
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
}
