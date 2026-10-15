// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor-only lowering and outward affine evaluation for conversion transactions.

use nrz_accuracy::Nonnegative;
use nrz_scalar::{
    ArithmeticContext, DecodedLiteral, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};
use nrz_units::{CheckedMap, DescriptorConstant};

use crate::conversion::ConversionError;

/// One exact map lowered once for a sealed format and arithmetic revision.
#[derive(Clone, Debug)]
pub(crate) struct LoweredMap<P: Scalar> {
    /// Original definition, including frozen declaration, reference and origin identities.
    pub(crate) exact: CheckedMap,
    /// Scale candidate, exact-constant enclosure and its own evaluation displacement.
    pub(crate) scale: DecodedLiteral<P>,
    /// Shift candidate and evaluation enclosure; radii never use this coefficient.
    pub(crate) shift: DecodedLiteral<P>,
    /// Arithmetic proof used by both coefficient decodes.
    pub(crate) revision: u32,
}

impl<P: Scalar> LoweredMap<P> {
    /// Export only descriptor constants; stored numerical data never enters units arithmetic.
    pub(crate) fn new(
        exact: &CheckedMap,
        limits: LiteralLimits,
        context: &ArithmeticContext,
    ) -> Result<Self, ConversionError> {
        Ok(Self {
            exact: exact.clone(),
            scale: lower(exact.scale(), limits, context)?,
            shift: lower(exact.shift(), limits, context)?,
            revision: context.revision(),
        })
    }

    /// Enclose the exact affine image and propagate the old radius exactly once.
    pub(crate) fn evaluate(
        &self,
        center: P,
        radius: P,
        context: &ArithmeticContext,
    ) -> Result<ConvertedEntry<P>, ConversionError> {
        let original = Finite::new(center)?;
        let bound = Nonnegative::new(radius)?;
        let candidate =
            Finite::new(self.scale.center().get() * original.get() + self.shift.center().get())?;
        let image = self
            .scale
            .enclosure()
            .mul(Enclosure::singleton(original), context)?
            .add(self.shift.enclosure(), context)?;
        let evaluation = Nonnegative::new(image.displacement_from(candidate, context)?.get())?;
        let magnitude = Nonnegative::new(self.scale.enclosure().abs().upper().get())?;
        let total = bound.scale(magnitude, context)?.add(evaluation, context)?;
        Ok(ConvertedEntry {
            center: candidate.get(),
            total: total.get(),
            evaluation: evaluation.get(),
        })
    }
}

/// Temporary kernel entry; callers collect each field into a separate private region.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ConvertedEntry<P: Scalar> {
    /// Finite candidate coordinate.
    pub(crate) center: P,
    /// Complete outward deterministic radius, including the carried input radius.
    pub(crate) total: P,
    /// New conversion evaluation error, overlapping the total rather than additive to it.
    pub(crate) evaluation: P,
}

/// Units owns the bounded exact export; scalar alone evaluates its numeric spelling.
#[expect(
    clippy::default_trait_access,
    reason = "the exact export policy type is inferred through units without a CAS dependency"
)]
fn lower<P: Scalar>(
    constant: &DescriptorConstant,
    limits: LiteralLimits,
    context: &ArithmeticContext,
) -> Result<DecodedLiteral<P>, ConversionError> {
    let spelling = constant.export(&Default::default())?;
    Ok(NumericLiteral::new(&spelling, limits)?.decode(context)?)
}
