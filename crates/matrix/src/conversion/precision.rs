// SPDX-License-Identifier: AGPL-3.0-only
//! Precision changes allocate new representations and preserve old deterministic coverage.

use super::{ConversionError, PrecisionEvidence};
use crate::binding::{NativeRecipe, buffer};
use crate::publication::Payload;
use crate::{
    Binding, CheckedOperation, CheckedShape, ExecutionLimits, Handle, NormalizedView,
    OperationDraft, OperationError, OutputReservation, RecipeEvidence, RegionContract, Value,
};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Precision, Scalar};
use nrz_units::Descriptor;

/// A region's actual coordinate descriptor and ordered tangent shape, retained unchanged by precision conversion.
#[derive(Clone, Debug)]
pub struct PrecisionContract {
    /// Checked input and output interpretation, independent of arena scalar format.
    region: RegionContract,
}

impl PrecisionContract {
    /// Validate a coordinate representation without evaluating or changing any unit constant.
    /// # Errors
    /// Refuses unsupported homogeneous shapes, incompatible descriptors and units failures.
    pub fn new(shape: CheckedShape, descriptor: &Descriptor) -> Result<Self, OperationError> {
        shape.declarations.conversion_map(descriptor, descriptor)?;
        Ok(Self {
            region: super::contract::region(shape, descriptor)?,
        })
    }
}

impl OperationDraft {
    /// Propose a complete bundle of new scalar-format representations.
    #[must_use]
    pub const fn precision(
        inputs: Vec<PrecisionContract>,
        source: Precision,
        target: Precision,
    ) -> Self {
        Self {
            inputs: Vec::new(),
            kernel: None,
            solve: None,
            conversions: None,
            precision: Some((inputs, source, target)),
        }
    }
}

impl<P: Scalar + 'static, Q: Scalar + 'static> Binding<P, Q> {
    /// Bind precision conversion to typed source handles and a distinct typed output format.
    ///
    /// Validation checks these types against the closed recipe's declared formats.
    #[must_use]
    pub const fn convert_precision(
        operation: CheckedOperation,
        inputs: Vec<Handle<Value, P>>,
        limits: ExecutionLimits,
    ) -> Self {
        Self {
            operation,
            inputs,
            limits,
            output: std::marker::PhantomData,
        }
    }
}

/// Check the two supported directions and reserve contracts under the normal execution policy.
pub(crate) fn check(
    contracts: Vec<PrecisionContract>,
    source: Precision,
    target: Precision,
    limits: ExecutionLimits,
) -> Result<CheckedOperation, OperationError> {
    if contracts.is_empty() {
        return Err(OperationError::InputCount);
    }
    if source == target {
        return Err(ConversionError::Contract.into());
    }
    let entries = contracts.iter().try_fold(0_usize, |total, contract| {
        total
            .checked_add(contract.region.shape().extent().entries())
            .ok_or(OperationError::ResourceLimit)
    })?;
    limits.check(contracts.len(), entries)?;
    let mut inputs = buffer(contracts.len())?;
    for contract in contracts {
        inputs.push(contract.region);
    }
    let inputs: std::rc::Rc<[RegionContract]> = inputs.into();
    Ok(CheckedOperation {
        outputs: std::rc::Rc::clone(&inputs),
        inputs,
        recipe: NativeRecipe::Precision { source, target },
        revision: 1,
    })
}

/// Evaluate all center displacements and outward-convert old bounds without adding them twice.
pub(crate) fn evaluate<P: Scalar + 'static, Q: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    output: OutputReservation,
    context: &ArithmeticContext,
    slot: usize,
) -> Result<(Payload<Q>, RecipeEvidence), OperationError> {
    let extent = view.extent();
    let mut centers = buffer(extent.entries())?;
    let mut totals = buffer(extent.entries())?;
    let mut errors = buffer(extent.entries())?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let (center, total, error) = convert::<P, Q>(
                view.get(row, column)?,
                view.bound(row, column)?.unwrap_or(P::ZERO),
                context,
            )?;
            centers.push(center);
            totals.push(total);
            errors.push(error);
        }
    }
    let claim = PrecisionEvidence {
        slot,
        entries: extent.entries(),
        coverage: output.coverage(),
        source: P::PRECISION,
        target: Q::PRECISION,
        revision: context.revision(),
    };
    Ok((
        Payload {
            extent,
            target: output.stamp(),
            centers,
            bounds: Some(totals),
            evaluation: Some(errors),
            contributions: None,
        },
        RecipeEvidence::Precision(claim),
    ))
}

/// Scalar owns exact widening and nearest/outward narrowing; matrix owns coverage composition.
fn convert<P: Scalar + 'static, Q: Scalar + 'static>(
    center: P,
    radius: P,
    context: &ArithmeticContext,
) -> Result<(Q, Q, Q), OperationError> {
    match (P::PRECISION, Q::PRECISION) {
        (Precision::F32, Precision::F64) => {
            let original = Finite::new(super::same_format::<P, f32>(center)?)
                .map_err(ConversionError::from)?;
            let change = nrz_scalar::widen(original, Enclosure::singleton(original));
            Ok((
                super::same_format(change.center().get())?,
                super::same_format(radius.to_f64())?,
                Q::ZERO,
            ))
        }
        (Precision::F64, Precision::F32) => {
            let (center, total, error) = narrow(center.to_f64(), radius.to_f64(), context)?;
            Ok((
                super::same_format(center)?,
                super::same_format(total)?,
                super::same_format(error)?,
            ))
        }
        _ => Err(ConversionError::Contract.into()),
    }
}

/// Add the new displacement once to the independently outward-converted prior total bound.
fn narrow(
    center: f64,
    radius: f64,
    context: &ArithmeticContext,
) -> Result<(f32, f32, f32), ConversionError> {
    let original = Finite::new(center)?;
    let old = Finite::new(radius)?;
    let change = nrz_scalar::narrow(original, Enclosure::singleton(original), context)?;
    let carried = nrz_scalar::narrow(old, Enclosure::singleton(old), context)?
        .enclosure()
        .upper();
    let error = Nonnegative::new(change.displacement().get())?;
    let total = Nonnegative::new(carried.get())?.add(error, context)?;
    Ok((change.center().get(), total.get(), error.get()))
}
