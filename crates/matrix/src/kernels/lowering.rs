// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed mathematical definitions are enclosed without routing heap values through bindings.

use super::linear::CoefficientValue;
use super::{FixedCoefficient, KernelError};
use crate::OperationError;
use nrz_scalar::{
    ArithmeticContext, Enclosure, Finite, LiteralLimits, NumericLiteral, Precision, Scalar,
};

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
    coefficient: &FixedCoefficient,
    limits: LiteralLimits,
    context: &ArithmeticContext,
) -> Result<Coefficient<P>, OperationError> {
    match &coefficient.value {
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
