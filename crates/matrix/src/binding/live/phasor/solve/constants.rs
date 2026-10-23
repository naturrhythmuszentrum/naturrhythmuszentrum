// SPDX-License-Identifier: AGPL-3.0-only
//! Exact fixed components require actual numerical coverage in every corresponding stored copy.

use super::{
    super::{AT, PhasorError, numeric::Values},
    PhasorSolve, transfer,
};
use nrz_bindings::Budget;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Optional exact real/imaginary enclosures for both original operands.
pub(super) type Fixed<P> = [[Option<Enclosure<P>>; 2]; 2];

/// Exact coefficient import precedes the first numerical source read on the same caller ledger.
pub(super) fn lower<P: Scalar>(
    plan: &PhasorSolve<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Fixed<P>, PhasorError> {
    let mut result = [[None; 2]; 2];
    for (operand, definitions) in result.iter_mut().zip(plan.definition().fixed_inputs()) {
        for (component, exact) in operand.iter_mut().zip(definitions) {
            if let Some(value) = exact {
                *component = Some(transfer::literal(value, plan.literals(), context, budget)?);
            }
        }
    }
    Ok(result)
}

/// Every fixed repeated, negative or zero copy must enclose the original exact constant in its own error.
pub(super) fn check<P: Scalar>(
    inputs: &[&Values<P>; 2],
    fixed: &Fixed<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(), PhasorError> {
    for (operand, definitions) in inputs.iter().zip(fixed) {
        for (index, component) in [0, 1, 1, 0].into_iter().enumerate() {
            budget.work(8, AT)?;
            if let Some(value) = definitions[component] {
                let value = if index == 2 { value.negated() } else { value };
                let required = value
                    .displacement_from(Finite::new(operand.centers[index])?, context)?
                    .get();
                if required > operand.errors[index] {
                    return Err(crate::LiveBindingError::UncoveredCopy.into());
                }
            }
        }
    }
    Ok(())
}
