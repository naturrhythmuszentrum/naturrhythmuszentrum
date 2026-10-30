// SPDX-License-Identifier: AGPL-3.0-only
//! Shared outward native boxes and inward comparisons with certified exact endpoints.

use super::LiveDecisionError;
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, Rational},
};
use nrz_scalar::{
    ArithmeticContext, ArithmeticFailure, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};

/// Decode only a certified exact endpoint; precharge the explicit bounded attempt on refusal too.
pub(in crate::publication) fn endpoint<P: Scalar>(
    exact: &Rational,
    arithmetic: &ArithmeticContext,
    literals: LiteralLimits,
    budget: &mut Budget,
    at: Location,
) -> Result<Enclosure<P>, LiveDecisionError> {
    let text = exact.exact_string(budget)?;
    budget.work(literals.max_work(), at)?;
    Ok(NumericLiteral::new(&text, literals)?
        .decode::<P>(arithmetic)?
        .enclosure())
}

/// The numerical error is already included in the total radius and is never added twice.
pub(in crate::publication) fn enclosure<P: Scalar>(
    coordinates: [P; 3],
    arithmetic: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let [center, total, _] = coordinates;
    let radius = Finite::new(total)?;
    Enclosure::singleton(Finite::new(center)?)
        .add(Enclosure::new(radius.negated().get(), total)?, arithmetic)
}
