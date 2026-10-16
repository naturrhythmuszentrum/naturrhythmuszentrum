// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical failures retain the missing source, arithmetic or target obligation.

/// A refused statistical propagation or covariance materialization.
#[derive(Clone, Debug, thiserror::Error)]
pub enum StatisticsError {
    /// A source model or its checked target relation refused.
    #[error("statistical source: {0}")]
    Joint(#[from] crate::JointError),
    /// A numerical region or current target could not be resolved.
    #[error("statistical storage: {0}")]
    Heap(#[from] crate::HeapError),
    /// Finite outward arithmetic could not establish a parameter enclosure.
    #[error("statistical arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// Units refused a paired covariance or sensitivity descriptor.
    #[error("statistical pairing: {0}")]
    Pairing(#[from] nrz_units::PairingError),
    /// The selected logical coordinate has no checked source-selection row.
    #[error("missing statistical source at operand {slot}, entry {entry}")]
    MissingSource {
        /// Ordered operand position.
        slot: usize,
        /// Logical column-order coordinate.
        entry: usize,
    },
    /// Distinct joint models provide no checked cross-model relation.
    #[error("statistical operands require one complete joint model")]
    MissingJointModel,
    /// This closed recipe has no implemented statistical propagation rule.
    #[error("unsupported statistical propagation recipe")]
    Unsupported,
    /// A private numerical or two-sided target invariant disagrees.
    #[error("statistical parameter or target contract mismatch")]
    Contract,
    /// Checked scalar-work, scratch or retained-record capacity was exhausted.
    #[error("statistical resource limit exhausted")]
    ResourceLimit,
}

impl From<nrz_scalar::ArithmeticFailure> for crate::OperationError {
    fn from(cause: nrz_scalar::ArithmeticFailure) -> Self {
        StatisticsError::Arithmetic(cause).into()
    }
}
impl From<crate::JointError> for crate::OperationError {
    fn from(cause: crate::JointError) -> Self {
        StatisticsError::Joint(cause).into()
    }
}
impl From<nrz_units::PairingError> for crate::OperationError {
    fn from(cause: nrz_units::PairingError) -> Self {
        StatisticsError::Pairing(cause).into()
    }
}
