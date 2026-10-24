// SPDX-License-Identifier: AGPL-3.0-only
//! Exact fixed components require actual numerical coverage in every corresponding stored copy.

use super::{
    super::{AT, PhasorError, numeric::Values},
    PhasorSolve, transfer,
};
use nrz_bindings::Budget;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Compact exact enclosures in original operand/component order; absent components allocate nothing.
pub(super) type Fixed<P> = Vec<Enclosure<P>>;

/// Exact coefficient import precedes the first numerical source read on the same caller ledger.
pub(super) fn lower<P: Scalar>(
    plan: &PhasorSolve<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Fixed<P>, PhasorError> {
    let definitions = plan.definition().fixed_inputs();
    let count = definitions
        .iter()
        .flatten()
        .filter(|value| value.is_some())
        .count();
    let mut result = crate::binding::buffer(count)?;
    for exact in definitions.iter().flatten().flatten() {
        result.push(transfer::literal(exact, plan.literals(), context, budget)?);
    }
    Ok(result)
}

/// Every fixed repeated, negative or zero copy must enclose the original exact constant in its own error.
pub(super) fn check<P: Scalar>(
    inputs: &[&Values<P>; 2],
    fixed: &[Enclosure<P>],
    present: [[bool; 2]; 2],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    let mut next = 0;
    for (operand, definitions) in inputs.iter().zip(present) {
        let selected = definitions.map(|present| {
            if present {
                let index = next;
                next += 1;
                Some(index)
            } else {
                None
            }
        });
        for (index, component) in [0, 1, 1, 0].into_iter().enumerate() {
            budget.work(8, AT)?;
            if let Some(slot) = selected[component] {
                let value = fixed.get(slot).ok_or(PhasorError::Identity)?;
                let value = if index == 2 { value.negated() } else { *value };
                let required = value
                    .displacement_from(Finite::new(operand.centers[index])?, context)?
                    .get();
                if required > operand.errors[index] {
                    return Err(crate::LiveBindingError::UncoveredCopy.into());
                }
            }
        }
    }
    if next != fixed.len() {
        return Err(PhasorError::Identity);
    }
    Ok(())
}
