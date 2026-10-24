// SPDX-License-Identifier: AGPL-3.0-only
//! Closed roles project their meanings from the original equation and exact selected sources.

use super::super::super::numeric::Values;
use super::super::{constants::Fixed, numeric::PhasorSolveCertificate};
use super::{PhasorError, Stage};
use nrz_scalar::{Enclosure, Scalar};
use std::any::TypeId;

/// One source copy's independently selected numerical channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhasorAllocationChannel {
    /// The actual selected stored center.
    Center,
    /// Its own deterministic total, including numerical error once.
    Total,
    /// Its own numerical-only error parameter.
    Error,
}
/// Exact closed native roles, interpreted through each allocation's original solve plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhasorAllocationStage {
    /// Both exact physical reference enclosures.
    References,
    /// Optional exact real/imaginary input components, in operand order.
    FixedInputs,
    /// Complete original impedance center/total/error block.
    Impedance,
    /// Complete original voltage center/total/error block.
    Voltage,
    /// One actual selected scalar source or companion, before assembling its block.
    InputCopy {
        /// Impedance zero or voltage one.
        operand: usize,
        /// Original logical column-major block position.
        copy: usize,
        /// Exact selected center or own-target companion channel.
        channel: PhasorAllocationChannel,
    },
    /// Approximate inverse of the original represented operator.
    InverseCandidate,
    /// Independently computed current block candidate.
    CurrentCandidate,
    /// Entrywise absolute bounds of I minus the inverse candidate times the operator.
    InverseDefects,
    /// Entrywise represented voltage minus operator times current candidate.
    CenterResiduals,
    /// Original selected impedance deterministic radii.
    OperatorTotals,
    /// Original selected voltage deterministic radii.
    VoltageTotals,
    /// Original selected impedance numerical radii.
    OperatorErrors,
    /// Original selected voltage numerical radii.
    VoltageErrors,
    /// Defect, candidate magnitude, inverse margin and inverse norm proof facts.
    InverseProof,
    /// Represented residual, candidate magnitude and forward radius proof facts.
    CenterProof,
    /// Complete common-operator deterministic perturbation proof facts.
    TotalProof,
    /// Complete common-operator numerical perturbation proof facts.
    NumericalProof,
    /// Final current centers, deterministic totals and numerical errors.
    Result,
    /// Final inverse, residual, margin and radius certificate fields.
    Certificate,
    /// Original metric operator, block and residual-transfer enclosures.
    Transfer,
    /// Numerical and total physical residual and current bounds.
    TransferredResidual,
    /// Reconstructed physical numerical residual's two norm inputs.
    NumericalNorm,
    /// Reconstructed physical total residual's two norm inputs.
    TotalNorm,
    /// Final current centers copied into the planned publication region.
    PublishedCenters,
    /// Final deterministic totals copied into their own-target publication region.
    PublishedTotals,
    /// Final numerical errors copied into their own-target publication region.
    PublishedErrors,
}
impl Stage {
    /// The finite schedule is independent of allocation addresses or equal byte counts.
    pub(super) fn at(slot: usize) -> Result<Self, PhasorError> {
        use PhasorAllocationChannel::{Center, Error, Total};
        const PREFIX: [Stage; 4] = [
            Stage::References,
            Stage::FixedInputs,
            Stage::Impedance,
            Stage::Voltage,
        ];
        const SUFFIX: [Stage; 21] = [
            Stage::InverseCandidate,
            Stage::CurrentCandidate,
            Stage::InverseDefects,
            Stage::CenterResiduals,
            Stage::OperatorTotals,
            Stage::VoltageTotals,
            Stage::OperatorErrors,
            Stage::VoltageErrors,
            Stage::InverseProof,
            Stage::CenterProof,
            Stage::TotalProof,
            Stage::NumericalProof,
            Stage::Result,
            Stage::Certificate,
            Stage::Transfer,
            Stage::TransferredResidual,
            Stage::NumericalNorm,
            Stage::TotalNorm,
            Stage::PublishedCenters,
            Stage::PublishedTotals,
            Stage::PublishedErrors,
        ];
        if slot < 4 {
            return Ok(PREFIX[slot]);
        }
        if slot < 28 {
            let index = slot - 4;
            return Ok(Self::InputCopy {
                operand: index / 12,
                copy: (index % 12) / 3,
                channel: [Center, Total, Error][index % 3],
            });
        }
        SUFFIX
            .get(slot - 28)
            .copied()
            .ok_or_else(|| crate::PrivateAllocationError::UnplannedMeaning.into())
    }
    /// Match exact type before entering any allocator or constructing an inline numerical owner.
    pub(super) const fn representation<P: Scalar + 'static>(self) -> (TypeId, usize) {
        use super::super::{PhasorTransferBounds, PhasorTransferredResidual};
        use crate::solve_verification::{CenterCertificate, FamilyCertificate, InverseWitness};
        match self {
            Self::References => (
                TypeId::of::<[Enclosure<P>; 2]>(),
                size_of::<[Enclosure<P>; 2]>(),
            ),
            Self::FixedInputs => (TypeId::of::<Fixed<P>>(), 4 * size_of::<Enclosure<P>>()),
            Self::Impedance | Self::Voltage | Self::Result => {
                (TypeId::of::<Values<P>>(), size_of::<Values<P>>())
            }
            Self::InverseCandidate | Self::CurrentCandidate => {
                (TypeId::of::<[P; 4]>(), size_of::<[P; 4]>())
            }
            Self::InverseProof => (
                TypeId::of::<InverseWitness<P>>(),
                size_of::<InverseWitness<P>>(),
            ),
            Self::CenterProof => (
                TypeId::of::<CenterCertificate<P>>(),
                size_of::<CenterCertificate<P>>(),
            ),
            Self::TotalProof | Self::NumericalProof => (
                TypeId::of::<FamilyCertificate<P>>(),
                size_of::<FamilyCertificate<P>>(),
            ),
            Self::Certificate => (
                TypeId::of::<PhasorSolveCertificate<P>>(),
                size_of::<PhasorSolveCertificate<P>>(),
            ),
            Self::Transfer => (
                TypeId::of::<PhasorTransferBounds<P>>(),
                size_of::<PhasorTransferBounds<P>>(),
            ),
            Self::TransferredResidual => (
                TypeId::of::<PhasorTransferredResidual<P>>(),
                size_of::<PhasorTransferredResidual<P>>(),
            ),
            Self::NumericalNorm | Self::TotalNorm => (TypeId::of::<[P; 2]>(), size_of::<[P; 2]>()),
            _ => (TypeId::of::<Vec<P>>(), self.entries() * size_of::<P>()),
        }
    }
    /// Maximum logical field count; allocation receipts omit absent fixed-input components.
    #[must_use]
    pub const fn entries(self) -> usize {
        match self {
            Self::InputCopy { .. } => 1,
            Self::References | Self::NumericalNorm | Self::TotalNorm => 2,
            Self::Impedance | Self::Voltage | Self::Result => 12,
            Self::Certificate => 9,
            Self::Transfer | Self::CenterProof => 3,
            Self::TotalProof | Self::NumericalProof => 5,
            _ => 4,
        }
    }
    /// Fixed owner bytes and observable vector capacities remain separate kinds of evidence.
    #[must_use]
    pub const fn is_vector(self) -> bool {
        matches!(
            self,
            Self::InputCopy { .. }
                | Self::FixedInputs
                | Self::InverseDefects
                | Self::CenterResiduals
                | Self::OperatorTotals
                | Self::VoltageTotals
                | Self::OperatorErrors
                | Self::VoltageErrors
                | Self::PublishedCenters
                | Self::PublishedTotals
                | Self::PublishedErrors
        )
    }
    /// Numerical owners shared by every completed evaluation clone.
    pub(super) const fn retained(self) -> bool {
        matches!(
            self,
            Self::Result | Self::Certificate | Self::Transfer | Self::TransferredResidual
        )
    }
}
