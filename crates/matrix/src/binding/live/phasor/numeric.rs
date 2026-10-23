// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed ordered two-term contractions cover every copy without asserting stochastic independence.

use super::{AT, PhasorError};
use nrz_bindings::Budget;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Fixed complete logical-column arrays keep all product copies independently rounded.
#[derive(Clone, Debug)]
pub(super) struct Values<P: Scalar> {
    /// Finite represented product centers.
    pub(super) centers: [P; 4],
    /// Full deterministic radii, including input and product rounding.
    pub(super) totals: [P; 4],
    /// Numerical-only radii relative to the original exact nominal family.
    pub(super) errors: [P; 4],
}

/// Execute the ordered Real matrix product attached to the exact correspondence.
pub(super) fn multiply<P: Scalar>(
    left: &Values<P>,
    right: &Values<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Values<P>, PhasorError> {
    let mut result = Values {
        centers: [P::ZERO; 4],
        totals: [P::ZERO; 4],
        errors: [P::ZERO; 4],
    };
    for column in 0..2 {
        for row in 0..2 {
            let index = row + 2 * column;
            let mut center = P::ZERO;
            for inner in 0..2 {
                budget.work(2, AT)?;
                let product =
                    Finite::new(left.centers[row + 2 * inner] * right.centers[inner + 2 * column])?;
                center = Finite::new(center + product.get())?.get();
            }
            result.centers[index] = center;
            result.totals[index] =
                radius(left, right, row, column, center, false, context, budget)?;
            result.errors[index] = radius(left, right, row, column, center, true, context, budget)?;
        }
    }
    Ok(result)
}

/// The same original centers have overlapping total and numerical-only coverage, never summed.
#[expect(
    clippy::too_many_arguments,
    reason = "fixed contraction indices and explicit coverage channel"
)]
fn radius<P: Scalar>(
    left: &Values<P>,
    right: &Values<P>,
    row: usize,
    column: usize,
    center: P,
    numerical: bool,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<P, PhasorError> {
    let mut sum = Enclosure::singleton(Finite::ZERO);
    let first = if numerical {
        &left.errors
    } else {
        &left.totals
    };
    let second = if numerical {
        &right.errors
    } else {
        &right.totals
    };
    for inner in 0..2 {
        budget.work(16, AT)?;
        let a = row + 2 * inner;
        let b = inner + 2 * column;
        let a = enclosure(left.centers[a], first[a], context)?;
        let b = enclosure(right.centers[b], second[b], context)?;
        sum = sum.add(a.mul(b, context)?, context)?;
    }
    Ok(sum.displacement_from(Finite::new(center)?, context)?.get())
}

/// Declared symmetric radii and each floating endpoint operation are outward enclosed.
fn enclosure<P: Scalar>(
    center: P,
    radius: P,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, PhasorError> {
    Ok(
        Enclosure::singleton(Finite::new(center)?)
            .add(Enclosure::new(-radius, radius)?, context)?,
    )
}
