// SPDX-License-Identifier: AGPL-3.0-only
//! Reconstruct a physical residual before applying the exact block/component norm transport.

use super::{
    super::{AT, PhasorError, numeric::Values},
    PhasorSolve,
};
use nrz_bindings::Budget;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Physical residual and current-error bounds in the explicitly retained voltage/current metrics.
///
/// These bounds concern the reconstructed first-column phasor and the original input family.
/// They never divide the verifier's independently rounded block residual by `sqrt(2)`.
#[derive(Clone, Copy, Debug)]
pub struct PhasorTransferredResidual<P: Scalar> {
    /// Numerical-only and total residual bounds in the requested voltage residual norm.
    pub residuals: [P; 2],
    /// Numerical-only and total component Euclidean current bounds in the named current metric.
    pub currents: [P; 2],
}

/// The true impedance is `i*X`; reconstruction uses that exact law and encloses original input copies.
pub(super) fn evaluate<P: Scalar>(
    plan: &PhasorSolve<P>,
    inputs: &[Values<P>; 2],
    candidate: &[P; 4],
    references: &[Enclosure<P>; 2],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<PhasorTransferredResidual<P>, PhasorError> {
    budget.work(256, AT)?;
    let mut residuals = [P::ZERO; 2];
    let mut currents = [P::ZERO; 2];
    let weight = point(P::from_u8(plan.transfer().weight()))?.sqrt(context)?;
    for (channel, numerical) in [true, false].into_iter().enumerate() {
        let impedance = coordinate(&inputs[0], 1, numerical, context)?;
        let real = coordinate(&inputs[1], 0, numerical, context)?
            .add(impedance.mul(point(candidate[1])?, context)?, context)?;
        let imaginary = coordinate(&inputs[1], 1, numerical, context)?
            .sub(impedance.mul(point(candidate[0])?, context)?, context)?;
        let magnitude = nrz_scalar::norm_upper(
            &[real.abs().upper().get(), imaginary.abs().upper().get()],
            context,
        )?
        .get();
        let normalized = point(magnitude)?
            .div(references[1], context)?
            .mul(weight, context)?;
        let factor = references[0].div(impedance.abs().mul(weight, context)?, context)?;
        residuals[channel] = normalized.upper().get();
        currents[channel] = normalized.mul(factor, context)?.upper().get();
    }
    Ok(PhasorTransferredResidual {
        residuals,
        currents,
    })
}

/// Preserve the original copy's selected total or numerical channel without double counting.
fn coordinate<P: Scalar>(
    values: &Values<P>,
    index: usize,
    numerical: bool,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, PhasorError> {
    let radius = if numerical {
        values.errors[index]
    } else {
        values.totals[index]
    };
    Ok(point(values.centers[index])?.add(Enclosure::new(-radius, radius)?, context)?)
}

/// Finite input singleton; every subsequent operation remains outward enclosed.
fn point<P: Scalar>(value: P) -> Result<Enclosure<P>, PhasorError> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}
