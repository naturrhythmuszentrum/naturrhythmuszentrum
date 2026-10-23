// SPDX-License-Identifier: AGPL-3.0-only
//! Closed bilinear coefficient checking over the actual ordered component projections.

use super::super::{ReadingError, support};
use nrz_cas::{Rational, limits::Budget, readings::AffineMap};

/// Prove every block entry by comparing exact coefficients of `(a*u,a*v,b*u,b*v)`.
///
/// These four products name the retained input projections, not new free declarations.
/// Distribution proves the identity even when some projections share an exact source.
pub(super) fn check(
    left: &AffineMap,
    right: &AffineMap,
    budget: &mut Budget,
) -> Result<AffineMap, ReadingError> {
    let mut actual = support::reserve(16, budget)?;
    for column in 0..2 {
        for row in 0..2 {
            for first in 0..2 {
                for second in 0..2 {
                    actual.push(entry(left, right, [row, column, first, second], budget)?);
                }
            }
        }
    }
    let expected = [1, 0, 0, -1, 0, 1, 1, 0, 0, -1, -1, 0, 1, 0, 0, -1];
    for (coefficient, integer) in actual.iter().zip(expected) {
        budget.work(1, support::AT)?;
        if coefficient != &Rational::from_ratio(integer, 1, budget)? {
            return Err(ReadingError::Relation);
        }
    }
    Ok(AffineMap::new(
        4,
        4,
        actual,
        vec![Rational::zero(); 4],
        budget,
    )?)
}

/// One ordered matrix contraction, expanded only by scalar distributivity.
fn entry(
    left: &AffineMap,
    right: &AffineMap,
    indices: [usize; 4],
    budget: &mut Budget,
) -> Result<Rational, ReadingError> {
    let [row, column, first, second] = indices;
    let mut result = Rational::zero();
    for inner in 0..2 {
        budget.work(1, support::AT)?;
        let a = left
            .coefficients()
            .get((row + 2 * inner) * 2 + first)
            .ok_or(ReadingError::Shape)?;
        let b = right
            .coefficients()
            .get((inner + 2 * column) * 2 + second)
            .ok_or(ReadingError::Shape)?;
        result = result.add(&a.mul(b, budget)?, budget)?;
    }
    Ok(result)
}
