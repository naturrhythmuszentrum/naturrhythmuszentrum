// SPDX-License-Identifier: AGPL-3.0-only
//! Exact image, original reuse identity and enclosed coefficients remain one immutable inventory.

use super::{CoordinateError, ReadingBinding};
use crate::ConstantCoefficient;
use nrz_bindings::{
    Budget,
    readings::{
        CoordinateImage,
        algebra::{Location, Rational, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, Scalar};

/// Each pair and result position is covered by the same ordered exact coordinate definition.
#[derive(Clone, Debug)]
pub(super) struct Lowered<P: Scalar> {
    /// Original native law, endpoint definitions, correspondence witness and complete key.
    pub(super) image: CoordinateImage,
    /// Scale and offset enclosures in exactly the image's per-entry order.
    pub(super) coefficients: Vec<[ConstantCoefficient<P>; 2]>,
    /// Certified constant owner, absent from ordinary rational coordinate paths.
    pub(super) pi: Option<crate::PiConstant<P>>,
}

impl<P: Scalar> Lowered<P> {
    /// Derive the numerical array size instead of storing a redundant counter beside its owner.
    pub(super) fn bytes(&self) -> Result<u64, CoordinateError> {
        u64::try_from(std::mem::size_of_val(self.coefficients.as_slice()))
            .map_err(|_| crate::HeapError::ExtentOverflow.into())
    }
}

/// Authenticate the complete original key and finish all coefficient imports before reading input floats.
pub(super) fn prepare<P: Scalar>(
    binding: &ReadingBinding<P>,
    image: &CoordinateImage,
    policy: LiteralLimits,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Lowered<P>, CoordinateError> {
    binding
        .group
        .graph
        .validate_reuse_key(image.reuse_key(), budget)?;
    let map = image.export(budget)?;
    let count = image.scales().len();
    let length = u64::try_from(count).map_err(|_| crate::HeapError::ExtentOverflow)?;
    let bytes = length
        .checked_mul(
            u64::try_from(std::mem::size_of::<[ConstantCoefficient<P>; 2]>())
                .map_err(|_| crate::HeapError::ExtentOverflow)?,
        )
        .ok_or(crate::HeapError::ExtentOverflow)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        bytes,
        Location::new("coordinate coefficients"),
    )?;
    let retained = map_bytes(&map)?
        .checked_add(image.retained_payload_bytes())
        .and_then(|value| value.checked_add(bytes))
        .ok_or(crate::HeapError::ExtentOverflow)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        retained,
        Location::new("coordinate image"),
    )?;
    let mut coefficients = crate::binding::buffer(count)?;
    for (index, offset) in map.offsets().iter().enumerate() {
        let scale = map
            .coefficients()
            .get(index * count + index)
            .ok_or(crate::HeapError::LengthMismatch)?;
        let pair = budget.with_retained_payload(
            retained,
            Location::new("coordinate coefficients"),
            |budget| {
                Ok::<_, CoordinateError>([
                    decode(scale, policy, context, budget)?,
                    decode(offset, policy, context, budget)?,
                ])
            },
        )?;
        if pair[0].enclosure().lower().get() <= P::ZERO {
            return Err(nrz_scalar::ArithmeticFailure::ZeroInDivisor.into());
        }
        coefficients.push(pair);
    }
    Ok(Lowered {
        image: image.clone(),
        coefficients,
        pi: None,
    })
}

/// Rational integer/fraction spelling has no exponent loop; this charge prepays every scan and operation.
fn decode<P: Scalar>(
    value: &Rational,
    policy: LiteralLimits,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<ConstantCoefficient<P>, CoordinateError> {
    let text = value.exact_string(budget)?;
    let length = u64::try_from(text.len()).map_err(|_| crate::HeapError::ExtentOverflow)?;
    let work = length
        .checked_mul(12)
        .and_then(|value| value.checked_add(8))
        .ok_or(crate::HeapError::ExtentOverflow)?;
    budget.work(work, Location::new("coordinate literal"))?;
    let limits = LiteralLimits::new(
        policy.max_digits(),
        policy.max_exponent(),
        policy.max_work().min(work),
    );
    Ok(ConstantCoefficient::literal(
        NumericLiteral::new(&text, limits)?.decode(context)?,
    ))
}

/// The exported rational rectangle remains live during coefficient decoding.
fn map_bytes(
    map: &nrz_bindings::readings::algebra::readings::AffineMap,
) -> Result<u64, CoordinateError> {
    let mut bytes = 0_u64;
    for value in map.coefficients().iter().chain(map.offsets()) {
        let (top, bottom) = value.component_bits();
        let limbs = top
            .div_ceil(64)
            .checked_add(bottom.div_ceil(64))
            .and_then(|value| value.checked_mul(16))
            .ok_or(crate::HeapError::ExtentOverflow)?;
        bytes = bytes
            .checked_add(limbs)
            .and_then(|value| {
                value.checked_add(u64::try_from(std::mem::size_of::<Rational>()).ok()?)
            })
            .ok_or(crate::HeapError::ExtentOverflow)?;
    }
    Ok(bytes)
}
