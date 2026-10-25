// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor-only lowering and outward affine evaluation for conversion transactions.

use nrz_accuracy::Nonnegative;
use nrz_bindings::{CheckedMap, DescriptorConstant};
use nrz_scalar::{
    ArithmeticContext, DecodedLiteral, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};
use std::rc::Rc;

use crate::conversion::ConversionError;

/// One exact map lowered once for a sealed format and arithmetic revision.
#[derive(Clone, Debug)]
pub(crate) struct LoweredMap<P: Scalar> {
    /// Original definition, including frozen declaration, reference and origin identities.
    pub(crate) exact: Rc<CheckedMap>,
    /// Scale candidate, exact-constant enclosure and its own evaluation displacement.
    pub(crate) scale: ConstantCoefficient<P>,
    /// Shift candidate and evaluation enclosure; radii never use this coefficient.
    pub(crate) shift: ConstantCoefficient<P>,
    /// Arithmetic proof used by both coefficient decodes.
    pub(crate) revision: u32,
}

impl<P: Scalar> LoweredMap<P> {
    /// Export only descriptor constants; stored numerical data never enters bindings arithmetic.
    pub(crate) fn new(
        exact: &Rc<CheckedMap>,
        limits: LiteralLimits,
        context: &ArithmeticContext,
    ) -> Result<Self, ConversionError> {
        Ok(Self {
            exact: Rc::clone(exact),
            scale: lower(exact.scale(), limits, context)?,
            shift: lower(exact.shift(), limits, context)?,
            revision: context.revision(),
        })
    }

    /// Propagate the earlier nominal-target error and add only this map's new discrepancy.
    pub(crate) fn propagate_error(
        &self,
        prior: P,
        discrepancy: P,
        context: &ArithmeticContext,
    ) -> Result<P, ConversionError> {
        let magnitude = Nonnegative::new(self.scale.enclosure().abs().upper().get())?;
        Ok(Nonnegative::new(prior)?
            .scale(magnitude, context)?
            .add(Nonnegative::new(discrepancy)?, context)?
            .get())
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

/// Bindings owns the bounded exact export; scalar alone evaluates its numeric spelling.
#[expect(
    clippy::default_trait_access,
    reason = "the exact export policy type is inferred through bindings without a CAS dependency"
)]
fn lower<P: Scalar>(
    constant: &DescriptorConstant,
    limits: LiteralLimits,
    context: &ArithmeticContext,
) -> Result<ConstantCoefficient<P>, ConversionError> {
    let spelling = constant.export(&Default::default())?;
    Ok(ConstantCoefficient::literal(
        NumericLiteral::new(&spelling, limits)?.decode(context)?,
    ))
}

/// Numerical fields of one certified coefficient, independent of its exact representation.
#[derive(Clone, Copy, Debug)]
pub struct ConstantCoefficient<P: Scalar> {
    /// Finite candidate used by the affine kernel.
    center: Finite<P>,
    /// Outward exact-definition bounds.
    enclosure: Enclosure<P>,
    /// Outward candidate displacement.
    displacement: Finite<P>,
    /// Complete charged coefficient lowering Work.
    work: u64,
}

impl<P: Scalar> ConstantCoefficient<P> {
    /// Preserve the ordinary rational literal decoder's center and error.
    pub(crate) const fn literal(decoded: DecodedLiteral<P>) -> Self {
        Self {
            center: decoded.center(),
            enclosure: decoded.enclosure(),
            displacement: decoded.displacement(),
            work: decoded.work_used(),
        }
    }
    /// Choose the lower endpoint and enclose the complete candidate displacement.
    pub(crate) fn enclosed(
        enclosure: Enclosure<P>,
        work: u64,
        context: &ArithmeticContext,
    ) -> Result<Self, ConversionError> {
        let center = enclosure.lower();
        Ok(Self {
            center,
            enclosure,
            displacement: enclosure.displacement_from(center, context)?,
            work,
        })
    }
    /// The finite numerical candidate.
    #[must_use]
    pub const fn center(self) -> Finite<P> {
        self.center
    }
    /// The complete outward coefficient family.
    #[must_use]
    pub const fn enclosure(self) -> Enclosure<P> {
        self.enclosure
    }
    /// A bound on every member's displacement from the candidate.
    #[must_use]
    pub const fn displacement(self) -> Finite<P> {
        self.displacement
    }
    /// Charged recognition, import and numerical lowering Work.
    #[must_use]
    pub const fn work_used(self) -> u64 {
        self.work
    }
}
