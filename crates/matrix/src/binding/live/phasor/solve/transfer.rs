// SPDX-License-Identifier: AGPL-3.0-only
//! Outward evaluation keeps the induced operator norm separate from the block Frobenius norm.

use super::{
    super::{AT, PhasorError},
    PhasorSolve,
};
use nrz_bindings::{Budget, readings::ReadingError};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar};

/// Enclosed exact metric factors for the retained nonzero-reactance family.
#[derive(Clone, Copy, Debug)]
pub struct PhasorTransferBounds<P: Scalar> {
    /// Induced Euclidean operator norm `abs(X)*I_ref/V_ref`.
    pub operator_two: Enclosure<P>,
    /// Frobenius block norm `sqrt(2)*abs(X)*I_ref/V_ref`.
    pub block_frobenius: Enclosure<P>,
    /// Residual-to-current factor in the requested residual and current component metrics.
    pub residual_factor: Enclosure<P>,
}

/// Import the exact retained reference quotient before reading any numerical operand.
pub(super) fn lower<P: Scalar>(
    plan: &PhasorSolve<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[Enclosure<P>; 2], PhasorError> {
    Ok([
        coefficient(
            plan.transfer().reference_ratio(),
            plan.literals(),
            context,
            budget,
        )?,
        coefficient(
            plan.transfer().frames()[0].entries()[0].magnitude(),
            plan.literals(),
            context,
            budget,
        )?,
    ])
}

/// One exact positive reference coefficient, with import work on the same caller ledger.
fn coefficient<P: Scalar>(
    value: &nrz_bindings::readings::algebra::ExactConstant,
    policy: LiteralLimits,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Enclosure<P>, PhasorError> {
    let rational = value.as_rational().ok_or(ReadingError::Units(
        nrz_bindings::UnitError::UnsupportedExactConstantExport,
    ))?;
    let result = literal(rational, policy, context, budget)?;
    if result.lower().get() <= P::ZERO {
        return Err(PhasorError::Identity);
    }
    Ok(result)
}

/// Import a fixed exact component; negative and zero constants remain valid.
pub(super) fn literal<P: Scalar>(
    rational: &nrz_bindings::readings::algebra::Rational,
    policy: LiteralLimits,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Enclosure<P>, PhasorError> {
    let text = rational.exact_string(budget)?;
    let work = u64::try_from(text.len())
        .map_err(|_| crate::HeapError::ExtentOverflow)?
        .checked_mul(12)
        .and_then(|size| size.checked_add(8))
        .ok_or(crate::HeapError::ExtentOverflow)?;
    budget.work(work, AT)?;
    let limits = LiteralLimits::new(
        policy.max_digits(),
        policy.max_exponent(),
        policy.max_work().min(work),
    );
    let result = NumericLiteral::new(&text, limits)?
        .decode::<P>(context)?
        .enclosure();
    Ok(result)
}

/// The exact block identity supplies the weight; source centers never choose a norm convention.
pub(super) fn evaluate<P: Scalar>(
    plan: &PhasorSolve<P>,
    ratio: Enclosure<P>,
    reactance: Enclosure<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<PhasorTransferBounds<P>, PhasorError> {
    budget.work(32, AT)?;
    let two = Enclosure::singleton(Finite::new(P::from_u8(2))?).sqrt(context)?;
    let weight =
        Enclosure::singleton(Finite::new(P::from_u8(plan.transfer().weight()))?).sqrt(context)?;
    let operator_two = reactance.div(ratio, context)?;
    Ok(PhasorTransferBounds {
        operator_two,
        block_frobenius: operator_two.mul(two, context)?,
        residual_factor: ratio.div(reactance.mul(weight, context)?, context)?,
    })
}
