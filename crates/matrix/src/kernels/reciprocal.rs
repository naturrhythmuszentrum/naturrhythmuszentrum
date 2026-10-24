// SPDX-License-Identifier: AGPL-3.0-only
//! Positive reciprocal boxes and fixed affine contraction form one private numerical bundle.

use super::{
    DomainReason, Elementwise, KernelError,
    arithmetic::{Entry, singleton},
};
use crate::private::{Buffer, Run};
use crate::{NormalizedView, OperationError};
use nrz_bindings::readings::ReciprocalStage;
use nrz_scalar::{
    ArithmeticContext, DecodedLiteral, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};

/// Meaning-free numerical schedule; exact definitions and correspondence remain beside the kernel.
#[derive(Clone, Debug)]
pub(crate) struct Schedule {
    /// Exact positive numerator spellings in input order.
    pub(crate) references: Vec<String>,
    /// Row-major affine coefficients over normalized reciprocals.
    pub(crate) coefficients: Vec<String>,
    /// One exact offset per column-ordered output entry.
    pub(crate) offsets: Vec<String>,
    /// Explicit bounded coefficient-import policy.
    pub(crate) literals: LiteralLimits,
}

/// Three images retain the original nominal target separately from its deterministic family.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Image<P: Scalar> {
    /// Rounded numerical candidate.
    center: P,
    /// Complete deterministic image.
    whole: Enclosure<P>,
    /// Image of the numerical-only input enclosure.
    nominal: Enclosure<P>,
}

/// Complete coefficient import precedes reading any numerical input or invoking a reciprocal.
pub(crate) fn evaluate<P: Scalar + 'static>(
    schedule: &Schedule,
    inputs: &[NormalizedView<'_, P>],
    context: &ArithmeticContext,
    allocations: &Run,
) -> Result<Buffer<Entry<P>>, OperationError> {
    let count = inputs.iter().try_fold(0_usize, |total, input| {
        total
            .checked_add(input.extent().entries())
            .ok_or(OperationError::ResourceLimit)
    })?;
    if count != schedule.references.len() {
        return Err(KernelError::Contract.into());
    }
    let references = decode::<P>(
        &schedule.references,
        schedule.literals,
        context,
        allocations,
        ReciprocalStage::References,
    )?;
    let coefficients = decode::<P>(
        &schedule.coefficients,
        schedule.literals,
        context,
        allocations,
        ReciprocalStage::Coefficients,
    )?;
    let offsets = decode::<P>(
        &schedule.offsets,
        schedule.literals,
        context,
        allocations,
        ReciprocalStage::Offsets,
    )?;
    let mut reciprocals = allocations.buffer(ReciprocalStage::Images)?;
    for input in inputs {
        for index in 0..input.extent().entries() {
            let entry = reciprocals.len();
            let numerator = *references.get(entry).ok_or(KernelError::Contract)?;
            reciprocals.push(reciprocal(input, index, numerator, entry, context)?)?;
        }
    }
    let mut output = allocations.buffer(ReciprocalStage::Contracted)?;
    for (row, offset) in offsets.iter().enumerate() {
        let start = row
            .checked_mul(count)
            .ok_or(OperationError::ResourceLimit)?;
        let end = start
            .checked_add(count)
            .ok_or(OperationError::ResourceLimit)?;
        let factors = coefficients.get(start..end).ok_or(KernelError::Contract)?;
        output.push(contract(&reciprocals, factors, *offset, context)?)?;
    }
    Ok(output)
}

/// Never replace a zero-containing or negative full box by the sign of its center.
fn reciprocal<P: Scalar + 'static>(
    input: &NormalizedView<'_, P>,
    index: usize,
    numerator: DecodedLiteral<P>,
    entry: usize,
    context: &ArithmeticContext,
) -> Result<Image<P>, OperationError> {
    let row = index % input.extent().rows();
    let column = index / input.extent().rows();
    let center = input.get(row, column)?;
    let total = input.bound(row, column)?.unwrap_or(P::ZERO);
    let error = input.evaluation(row, column)?.unwrap_or(P::ZERO);
    if error < P::ZERO || error > total {
        return Err(KernelError::Contract.into());
    }
    let whole = box_image(center, total, context)?;
    if whole.lower().get() <= P::ZERO || numerator.enclosure().lower().get() <= P::ZERO {
        return Err(KernelError::Domain {
            function: Elementwise::Reciprocal,
            entry,
            reason: DomainReason::NonpositiveInput,
        }
        .into());
    }
    let nominal = box_image(center, error, context)?;
    Ok(Image {
        center: Finite::new(numerator.center().get() / center)
            .map_err(KernelError::from)?
            .get(),
        whole: numerator
            .enclosure()
            .div(whole, context)
            .map_err(KernelError::from)?,
        nominal: numerator
            .enclosure()
            .div(nominal, context)
            .map_err(KernelError::from)?,
    })
}

/// The same exact coefficient enclosure acts on both channels without double-counting errors.
fn contract<P: Scalar>(
    inputs: &[Image<P>],
    factors: &[DecodedLiteral<P>],
    offset: DecodedLiteral<P>,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    let mut image = Image {
        center: offset.center().get(),
        whole: offset.enclosure(),
        nominal: offset.enclosure(),
    };
    for (input, coefficient) in inputs.iter().zip(factors) {
        image.center = Finite::new(image.center + coefficient.center().get() * input.center)?.get();
        image.whole = image
            .whole
            .add(coefficient.enclosure().mul(input.whole, context)?, context)?;
        image.nominal = image.nominal.add(
            coefficient.enclosure().mul(input.nominal, context)?,
            context,
        )?;
    }
    let center = Finite::new(image.center)?;
    let total = image.whole.displacement_from(center, context)?.get();
    let error = image.nominal.displacement_from(center, context)?.get();
    Ok(Entry {
        center: center.get(),
        total: if total < error { error } else { total },
        error,
    })
}

/// Outward endpoint construction preserves the complete input radius, including import error.
fn box_image<P: Scalar>(
    center: P,
    radius: P,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, KernelError> {
    let center = singleton(center)?;
    let radius = singleton(radius)?;
    Ok(Enclosure::new(
        center.sub(radius, context)?.lower().get(),
        center.add(radius, context)?.upper().get(),
    )?)
}

/// Rational spelling scans and primitive decoding fit the predeclared finite schedule.
fn decode<P: Scalar + 'static>(
    values: &[String],
    limits: LiteralLimits,
    context: &ArithmeticContext,
    allocations: &Run,
    stage: ReciprocalStage,
) -> Result<Buffer<DecodedLiteral<P>>, OperationError> {
    let mut result = allocations.buffer(stage)?;
    for text in values {
        let work = u64::try_from(text.len())
            .map_err(|_| OperationError::ResourceLimit)?
            .checked_mul(12)
            .and_then(|value| value.checked_add(8))
            .ok_or(OperationError::ResourceLimit)?;
        let policy = LiteralLimits::new(
            limits.max_digits(),
            limits.max_exponent(),
            limits.max_work().min(work),
        );
        result.push(
            NumericLiteral::new(text, policy)
                .map_err(KernelError::from)?
                .decode(context)
                .map_err(KernelError::from)?,
        )?;
    }
    Ok(result)
}
