// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed mathematical definitions are enclosed without routing heap values through bindings.

use super::KernelError;
use crate::OperationError;
use nrz_bindings::DescriptorConstant;
use nrz_scalar::{
    ArithmeticContext, Enclosure, Finite, LiteralLimits, NumericLiteral, Precision, Scalar,
};

/// Definitions cross the descriptor boundary before any numerical operand is read.
#[derive(Clone, Debug)]
pub(crate) enum CoefficientValue {
    /// Checked source-free structural metadata, including signed and zero exact coefficients.
    Expansion(Box<nrz_bindings::Conversion>),
    /// This finite binary64 value defines the exact mathematical coefficient.
    Represented(f64),
    /// Bindings-owned exact descriptor metadata, lowered under explicit limits.
    Descriptor(Box<DescriptorConstant>),
}

/// A fixed coefficient's finite candidate and independent exact-definition enclosure.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Coefficient<P: Scalar> {
    /// Candidate used in the ordinary floating kernel.
    pub(crate) center: P,
    /// Enclosure of the exact mathematical coefficient.
    pub(crate) enclosure: Enclosure<P>,
}

/// Lower each definition independently and retain coefficient uncertainty over perturbations.
#[expect(
    clippy::default_trait_access,
    reason = "exact export limits are inferred through the public bindings facade"
)]
pub(crate) fn lower<P: Scalar + 'static>(
    coefficient: &CoefficientValue,
    limits: LiteralLimits,
    context: &ArithmeticContext,
) -> Result<Coefficient<P>, OperationError> {
    match coefficient {
        CoefficientValue::Expansion(value) => {
            crate::statistics::lower_definition(value, limits, context)
        }
        CoefficientValue::Represented(value) => represented(*value, context),
        CoefficientValue::Descriptor(value) => {
            let spelling = value.export(&Default::default())?;
            let decoded = NumericLiteral::new(&spelling, limits)
                .map_err(KernelError::from)?
                .decode(context)
                .map_err(KernelError::from)?;
            Ok(Coefficient {
                center: decoded.center().get(),
                enclosure: decoded.enclosure(),
            })
        }
    }
}

/// Narrow fixed binary64 definitions outward when executing in binary32.
fn represented<P: Scalar + 'static>(
    value: f64,
    context: &ArithmeticContext,
) -> Result<Coefficient<P>, OperationError> {
    let source = Finite::new(value).map_err(KernelError::from)?;
    let (center, lower, upper) = match P::PRECISION {
        Precision::F64 => (
            crate::conversion::same_format(value)?,
            crate::conversion::same_format(value)?,
            crate::conversion::same_format(value)?,
        ),
        Precision::F32 => {
            let decoded = nrz_scalar::narrow(source, Enclosure::singleton(source), context)
                .map_err(KernelError::from)?;
            (
                crate::conversion::same_format(decoded.center().get())?,
                crate::conversion::same_format(decoded.enclosure().lower().get())?,
                crate::conversion::same_format(decoded.enclosure().upper().get())?,
            )
        }
    };
    Ok(Coefficient {
        center,
        enclosure: Enclosure::new(lower, upper).map_err(KernelError::from)?,
    })
}
