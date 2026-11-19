// SPDX-License-Identifier: AGPL-3.0-only
//! Closed version-one outcome codes preserve the exact consuming operation and cause.
use crate::loan::lc_origin::LcOutcome;
use crate::loan::lc_shape::readers::LcKernelStage;
use crate::{
    BinaryElementwise, DomainReason, Elementwise, KernelError, LcFrequencyTarget, LcProductError,
    LcRequestedState,
};
use nrz_accuracy::AccuracyFailure;
use nrz_scalar::{ArithmeticFailure, Scalar};

/// Convert only supported numerical outcomes; the original phase error remains structured.
pub(super) fn convert<P: Scalar>(
    result: Result<LcRequestedState<P>, (LcKernelStage, LcProductError)>,
    cell: usize,
) -> Result<(LcOutcome, Option<[P; 3]>), LcProductError> {
    match result {
        Ok(LcRequestedState::Ready {
            rate: Some(rate),
            values,
            ..
        }) if values.omega == Some(rate)
            && values.frequency.is_none()
            && values.period.is_none() =>
        {
            Ok((
                LcOutcome {
                    status: 2,
                    target: 0,
                    reason: 0,
                },
                Some(rate),
            ))
        }
        Ok(LcRequestedState::Ready { .. }) => Err(KernelError::Contract.into()),
        Ok(LcRequestedState::Invalid(target)) => {
            let target = match target {
                LcFrequencyTarget::Inductance => 1,
                LcFrequencyTarget::Capacitance => 2,
                LcFrequencyTarget::Product | LcFrequencyTarget::Root => {
                    return Err(KernelError::Contract.into());
                }
            };
            Ok((
                LcOutcome {
                    status: 3,
                    target,
                    reason: 1,
                },
                None,
            ))
        }
        Ok(LcRequestedState::Undecided(target)) => {
            let target = match target {
                LcFrequencyTarget::Inductance => 1,
                LcFrequencyTarget::Capacitance => 2,
                LcFrequencyTarget::Product => 3,
                LcFrequencyTarget::Root => 4,
            };
            Ok((
                LcOutcome {
                    status: 4,
                    target,
                    reason: 2,
                },
                None,
            ))
        }
        Err((stage, error)) => {
            let Some(reason) = reason(stage, &error, cell) else {
                return Err(error);
            };
            let target = match stage {
                LcKernelStage::Product => 3,
                LcKernelStage::Root => 4,
                LcKernelStage::Reciprocal => 5,
            };
            Ok((
                LcOutcome {
                    status: 5,
                    target,
                    reason,
                },
                None,
            ))
        }
    }
}
/// Function and original entry agreement precede any encoding of a domain error.
fn reason(stage: LcKernelStage, error: &LcProductError, cell: usize) -> Option<u16> {
    let LcProductError::Kernel(error) = error else {
        return None;
    };
    match error {
        KernelError::BinaryDomain {
            function,
            entry,
            reason,
        } => (stage == LcKernelStage::Product
            && *function == BinaryElementwise::Product
            && *entry == cell
            && *reason == DomainReason::UnboundedImage)
            .then_some(0x0104),
        KernelError::Domain {
            function,
            entry,
            reason,
        } => {
            if *entry != cell {
                return None;
            }
            match (stage, function, reason) {
                (LcKernelStage::Root, Elementwise::SquareRoot, DomainReason::NegativeRoot) => {
                    Some(0x0103)
                }
                (LcKernelStage::Root, Elementwise::SquareRoot, DomainReason::UnboundedImage)
                | (
                    LcKernelStage::Reciprocal,
                    Elementwise::Reciprocal,
                    DomainReason::UnboundedImage,
                ) => Some(0x0104),
                (
                    LcKernelStage::Reciprocal,
                    Elementwise::Reciprocal,
                    DomainReason::ZeroInDivisor,
                ) => Some(0x0102),
                _ => None,
            }
        }
        KernelError::Arithmetic(cause) => scalar(stage, *cause).map(|code| 0x0200 + code),
        KernelError::Accuracy(cause) => match cause {
            AccuracyFailure::InvalidBound => Some(0x0301),
            AccuracyFailure::Arithmetic(cause) => scalar(stage, *cause).map(|code| 0x0400 + code),
            AccuracyFailure::LengthMismatch
            | AccuracyFailure::InvalidBudget
            | AccuracyFailure::InvalidPolicy
            | AccuracyFailure::InvalidRecoveryState
            | AccuracyFailure::ResourceLimit(_) => None,
        },
        KernelError::Quantity { .. } | KernelError::Contract | KernelError::CoverageMismatch => {
            None
        }
    }
}
/// Scalar causes are admitted only at the operation whose guard supports them.
const fn scalar(stage: LcKernelStage, cause: ArithmeticFailure) -> Option<u16> {
    match cause {
        ArithmeticFailure::NonFinite => Some(1),
        ArithmeticFailure::InvertedEndpoints => Some(2),
        ArithmeticFailure::Overflow => Some(5),
        ArithmeticFailure::ZeroInDivisor => match stage {
            LcKernelStage::Reciprocal => Some(3),
            LcKernelStage::Product | LcKernelStage::Root => None,
        },
        ArithmeticFailure::NegativeRoot => match stage {
            LcKernelStage::Root => Some(4),
            LcKernelStage::Product | LcKernelStage::Reciprocal => None,
        },
        ArithmeticFailure::InvalidLiteral
        | ArithmeticFailure::LiteralRange
        | ArithmeticFailure::ResourceLimit
        | ArithmeticFailure::LengthMismatch
        | ArithmeticFailure::UnsupportedEnvironment => None,
    }
}
