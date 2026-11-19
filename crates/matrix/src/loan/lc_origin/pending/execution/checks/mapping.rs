// SPDX-License-Identifier: AGPL-3.0-only
//! Literal target/cause tables distinguish reachable cells from defensive conversion cases.
use super::super::outcome::convert;
use super::TestResult;
use crate::loan::lc_shape::readers::LcKernelStage;
use crate::{
    BinaryElementwise, DomainReason, Elementwise, KernelError, LcFrequencyTarget, LcProductError,
    LcRequestedState, LcRequestedValues, LoanError,
};
use nrz_accuracy::AccuracyFailure;
use nrz_scalar::{ArithmeticFailure, Scalar};
const STAGES: [LcKernelStage; 3] = [
    LcKernelStage::Product,
    LcKernelStage::Root,
    LcKernelStage::Reciprocal,
];
const SCALARS: [ArithmeticFailure; 10] = [
    ArithmeticFailure::NonFinite,
    ArithmeticFailure::InvertedEndpoints,
    ArithmeticFailure::ZeroInDivisor,
    ArithmeticFailure::NegativeRoot,
    ArithmeticFailure::Overflow,
    ArithmeticFailure::InvalidLiteral,
    ArithmeticFailure::LiteralRange,
    ArithmeticFailure::ResourceLimit,
    ArithmeticFailure::LengthMismatch,
    ArithmeticFailure::UnsupportedEnvironment,
];

pub(super) fn check() -> TestResult {
    format::<f32>()?;
    format::<f64>()
}
fn format<P: Scalar>() -> TestResult {
    scalar_codes::<P>()?;
    domain_codes::<P>()?;
    states::<P>()?;
    ready::<P>()?;
    for stage in STAGES {
        for error in [
            KernelError::Contract,
            KernelError::CoverageMismatch,
            KernelError::Quantity {
                cause: nrz_scalar::KernelError::WorkLimit,
                work: 7,
            },
        ] {
            rejected::<P>(stage, error.into());
        }
        rejected::<P>(stage, LoanError::WrongLoan.into());
        for cause in [
            AccuracyFailure::LengthMismatch,
            AccuracyFailure::InvalidBudget,
            AccuracyFailure::InvalidPolicy,
            AccuracyFailure::InvalidRecoveryState,
            AccuracyFailure::ResourceLimit(nrz_accuracy::ResourceLimit::Work),
            AccuracyFailure::ResourceLimit(nrz_accuracy::ResourceLimit::Attempts),
        ] {
            rejected::<P>(stage, KernelError::Accuracy(cause).into());
        }
        encoded::<P>(
            stage,
            KernelError::Accuracy(AccuracyFailure::InvalidBound),
            0x0301,
        )?;
    }
    Ok(())
}
fn scalar_codes<P: Scalar>() -> TestResult {
    // Each row is a target's complete independently specified scalar suffix table.
    for (stage, allowed) in STAGES.into_iter().zip([
        [true, true, false, false, true],
        [true, true, false, true, true],
        [true, true, true, false, true],
    ]) {
        for (index, cause) in SCALARS.into_iter().enumerate() {
            for base in [0x0200_u16, 0x0400] {
                let error = if base == 0x0200 {
                    KernelError::Arithmetic(cause)
                } else {
                    KernelError::Accuracy(AccuracyFailure::Arithmetic(cause))
                };
                if allowed.get(index).copied().unwrap_or(false) {
                    encoded::<P>(stage, error, base + u16::try_from(index + 1)?)?;
                } else {
                    rejected::<P>(stage, error.into());
                }
            }
        }
    }
    Ok(())
}
fn domain_codes<P: Scalar>() -> TestResult {
    for stage in STAGES {
        for (reason, code) in [
            (DomainReason::NonpositiveInput, 0x0101),
            (DomainReason::ZeroInDivisor, 0x0102),
            (DomainReason::NegativeRoot, 0x0103),
            (DomainReason::UnboundedImage, 0x0104),
        ] {
            for entry in [2, 3] {
                for function in [
                    Elementwise::Square,
                    Elementwise::SquareRoot,
                    Elementwise::Reciprocal,
                ] {
                    let supported = entry == 3
                        && matches!(
                            (stage, function, reason),
                            (
                                LcKernelStage::Root,
                                Elementwise::SquareRoot,
                                DomainReason::NegativeRoot | DomainReason::UnboundedImage
                            ) | (
                                LcKernelStage::Reciprocal,
                                Elementwise::Reciprocal,
                                DomainReason::ZeroInDivisor | DomainReason::UnboundedImage
                            )
                        );
                    let error = KernelError::Domain {
                        function,
                        entry,
                        reason,
                    };
                    if supported {
                        encoded::<P>(stage, error, code)?;
                    } else {
                        rejected::<P>(stage, error.into());
                    }
                }
                for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
                    let supported = entry == 3
                        && stage == LcKernelStage::Product
                        && function == BinaryElementwise::Product
                        && reason == DomainReason::UnboundedImage;
                    let error = KernelError::BinaryDomain {
                        function,
                        entry,
                        reason,
                    };
                    if supported {
                        encoded::<P>(stage, error, code)?;
                    } else {
                        rejected::<P>(stage, error.into());
                    }
                }
            }
        }
    }
    Ok(())
}
fn encoded<P: Scalar>(stage: LcKernelStage, error: KernelError, code: u16) -> TestResult {
    let (record, values) = convert::<P>(Err((stage, error.into())), 3)?;
    let target = match stage {
        LcKernelStage::Product => 3,
        LcKernelStage::Root => 4,
        LcKernelStage::Reciprocal => 5,
    };
    assert_eq!(
        (record.status, record.target, record.reason),
        (5, target, code)
    );
    assert_eq!(values, None);
    Ok(())
}
fn rejected<P: Scalar>(stage: LcKernelStage, error: LcProductError) {
    let expected = format!("{error:?}");
    let Err(actual) = convert::<P>(Err((stage, error)), 3) else {
        panic!("unsupported cause encoded");
    };
    assert_eq!(format!("{actual:?}"), expected);
}
fn states<P: Scalar>() -> TestResult {
    for (target, code) in [
        (LcFrequencyTarget::Inductance, 1),
        (LcFrequencyTarget::Capacitance, 2),
        (LcFrequencyTarget::Product, 3),
        (LcFrequencyTarget::Root, 4),
    ] {
        let (record, values) = convert::<P>(Ok(LcRequestedState::Undecided(target)), 3)?;
        assert_eq!((record.status, record.target, record.reason), (4, code, 2));
        assert_eq!(values, None);
        let invalid = convert::<P>(Ok(LcRequestedState::Invalid(target)), 3);
        if code <= 2 {
            let (record, values) = invalid?;
            assert_eq!((record.status, record.target, record.reason), (3, code, 1));
            assert_eq!(values, None);
        } else {
            assert!(matches!(
                invalid,
                Err(LcProductError::Kernel(KernelError::Contract))
            ));
        }
    }
    for (rate, omega) in [
        (None, None),
        (Some([P::ONE, P::ZERO, P::ZERO]), None),
        (None, Some([P::ONE, P::ZERO, P::ZERO])),
    ] {
        let result = convert::<P>(
            Ok(LcRequestedState::Ready {
                tau: [P::ONE, P::ZERO, P::ZERO],
                rate,
                values: LcRequestedValues {
                    omega,
                    frequency: None,
                    period: None,
                },
            }),
            3,
        );
        assert!(matches!(
            result,
            Err(LcProductError::Kernel(KernelError::Contract))
        ));
    }
    Ok(())
}

fn ready<P: Scalar>() -> TestResult {
    let rate = [P::ONE, P::ZERO, P::ZERO];
    for variant in 0..4 {
        let values = LcRequestedValues {
            omega: Some(if variant == 1 {
                [P::from_u8(2), P::ZERO, P::ZERO]
            } else {
                rate
            }),
            frequency: (variant == 2).then_some(rate),
            period: (variant == 3).then_some(rate),
        };
        let result = convert::<P>(
            Ok(LcRequestedState::Ready {
                tau: rate,
                rate: Some(rate),
                values,
            }),
            3,
        );
        if variant == 0 {
            let (record, values) = result?;
            assert_eq!((record.status, record.target, record.reason), (2, 0, 0));
            assert_eq!(values, Some(rate));
        } else {
            assert!(matches!(
                result,
                Err(LcProductError::Kernel(KernelError::Contract))
            ));
        }
    }
    Ok(())
}
