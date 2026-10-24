// SPDX-License-Identifier: AGPL-3.0-only
//! Outward positive products, roots and time restoration cover the complete represented family.

use super::{AT, HeapError, LcSpectrum, SpectralError as Error, SpectralValue};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{
    ArithmeticContext, DecodedLiteral, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};

/// Literal decoding is completely paid before any numerical source is read.
pub(super) fn constants<P: Scalar>(
    plan: &LcSpectrum<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[DecodedLiteral<P>; 3], Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(3)
        .map_err(|_| HeapError::Capacity)?;
    for text in &plan.0.constants {
        let work = u64::try_from(text.len())
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(12)
            .and_then(|count| count.checked_add(8))
            .ok_or(HeapError::ExtentOverflow)?;
        budget.work(work, AT)?;
        let policy = plan.0.literals;
        let limits = LiteralLimits::new(
            policy.max_digits(),
            policy.max_exponent(),
            policy.max_work().min(work),
        );
        let value = NumericLiteral::new(text, limits)?.decode(context)?;
        positive(value.enclosure(), budget)?;
        result.push(value);
    }
    result.try_into().map_err(|_| Error::Identity)
}

/// Center, full-family and original nominal schedules follow the same exact ordered law.
pub(super) fn evaluate<P: Scalar>(
    inputs: &[SpectralValue<P>; 2],
    constants: &[DecodedLiteral<P>; 3],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[SpectralValue<P>; 3], Error> {
    budget.check(
        BudgetKind::StagedPayloadBytes,
        u64::try_from(24 * std::mem::size_of::<Enclosure<P>>())
            .map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    let whole = boxes(inputs, false, context, budget)?;
    let nominal = boxes(inputs, true, context, budget)?;
    let full = image(&whole, constants, context, budget)?;
    let exact = image(&nominal, constants, context, budget)?;
    budget.work(7, AT)?;
    let upper = Finite::new(constants[0].center().get() / inputs[1].center)?.get();
    let lower = Finite::new(constants[1].center().get() / inputs[0].center)?.get();
    let magnitude = Finite::new(Finite::new(upper * lower)?.get().sqrt())?.get();
    let centers = [
        magnitude,
        Finite::new(magnitude / constants[2].center().get())?.get(),
        Finite::new(
            Finite::new(inputs[0].center / inputs[1].center)?
                .get()
                .sqrt(),
        )?
        .get(),
    ];
    let mut result = [SpectralValue {
        center: P::ZERO,
        total: P::ZERO,
        error: P::ZERO,
    }; 3];
    for (index, center) in centers.into_iter().enumerate() {
        budget.work(6, AT)?;
        let center = Finite::new(center)?;
        let total = full[index].displacement_from(center, context)?.get();
        let error = exact[index].displacement_from(center, context)?.get();
        result[index] = SpectralValue {
            center: center.get(),
            total: if total < error { error } else { total },
            error,
        };
    }
    Ok(result)
}

/// Both total and numerical-only boxes must be positive; a center sign cannot replace separation.
fn boxes<P: Scalar>(
    inputs: &[SpectralValue<P>; 2],
    numerical: bool,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[Enclosure<P>; 2], Error> {
    let mut result = [Enclosure::singleton(Finite::ZERO); 2];
    for (index, input) in inputs.iter().enumerate() {
        budget.work(3, AT)?;
        let radius = if numerical { input.error } else { input.total };
        let value = Enclosure::singleton(Finite::new(input.center)?)
            .add(Enclosure::new(-radius, radius)?, context)?;
        if value.upper().get() <= P::ZERO {
            return Err(Error::NonpositiveInput { input: index });
        }
        if value.lower().get() <= P::ZERO {
            return Err(Error::InputNotSeparated { input: index });
        }
        result[index] = value;
    }
    Ok(result)
}

/// Both normalized off-diagonal coefficients and their product need strict outward separation.
fn image<P: Scalar>(
    inputs: &[Enclosure<P>; 2],
    constants: &[DecodedLiteral<P>; 3],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[Enclosure<P>; 3], Error> {
    budget.work(24, AT)?;
    let upper = constants[0].enclosure().div(inputs[1], context)?;
    let lower = constants[1].enclosure().div(inputs[0], context)?;
    positive(upper, budget)?;
    positive(lower, budget)?;
    let product = upper.mul(lower, context)?;
    positive(product, budget)?;
    let magnitude = product.sqrt(context)?;
    let restored = magnitude.div(constants[2].enclosure(), context)?;
    let impedance = inputs[0].div(inputs[1], context)?.sqrt(context)?;
    positive(magnitude, budget)?;
    positive(restored, budget)?;
    positive(impedance, budget)?;
    Ok([magnitude, restored, impedance])
}

/// Insufficient separation remains a named refusal even when a positive center was representable.
fn positive<P: Scalar>(value: Enclosure<P>, budget: &mut Budget) -> Result<(), Error> {
    budget.work(1, AT)?;
    if value.lower().get() <= P::ZERO {
        return Err(Error::NotSeparated);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, positive};
    use nrz_bindings::{Budget, Limits};
    use nrz_scalar::Enclosure;

    /// An underflowed lower endpoint cannot prove a strictly positive product or restored rate.
    /// ´test:unit:lc-spectral-positive-guard-is-strict´
    #[test]
    fn lc_spectral_positive_guard_is_strict() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(Limits::default())?;
        assert!(matches!(
            positive(Enclosure::new(0.0_f64, 1.0)?, &mut budget),
            Err(Error::NotSeparated)
        ));
        positive(Enclosure::new(f64::from_bits(1), 1.0)?, &mut budget)?;
        Ok(())
    }
}
