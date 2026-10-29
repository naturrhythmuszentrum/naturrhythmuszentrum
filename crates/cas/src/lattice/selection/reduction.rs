// SPDX-License-Identifier: AGPL-3.0-only
//! Supplied integral inverses transport orthogonal comparisons in the original index order.

use super::{LatticeEnclosure, LatticeSelection};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;
use std::cmp::Ordering;

/// Reduction-specific arithmetic never charges an ordinary floor or orthogonal call.
const AT: Location = Location::new("lattice reduction nearest");

/// Exact finite certificate; no search for a reduction or alternative candidate occurs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeReduction {
    /// Supplied row-major integral U, defining the reduced basis B*U.
    pub transform: Vec<Rational>,
    /// Supplied row-major integral W, checked as both inverses of U.
    pub inverse: Vec<Rational>,
    /// Proposed reduced Integer tuple m, checked against original k = U*m.
    pub values: Vec<Handle>,
    /// One original-target box in the original basis; nested reductions are unsupported.
    pub enclosure: LatticeEnclosure,
}

/// Invalid reduction data never supplies nearest-selection authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Integral two-sided inverses prove det(U) = +/-1; the computed B*U must be orthogonal.
pub(super) fn validate(
    cas: &Cas,
    selection: &LatticeSelection,
    reduction: &LatticeReduction,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = selection.values.len();
    let slots = count.checked_mul(count).ok_or_else(invalid)?;
    if reduction.transform.len() != slots
        || reduction.inverse.len() != slots
        || reduction.values.len() != count
        || matches!(reduction.enclosure, LatticeEnclosure::Reduced(_))
    {
        return Err(invalid());
    }
    for value in reduction.transform.iter().chain(&reduction.inverse) {
        budget.work(1, AT)?;
        value.check(budget, AT)?;
        if !value.is_integer() {
            return Err(invalid());
        }
    }
    for value in &reduction.values {
        super::literal(cas, value, &selection.cover.index.basis.context)?;
    }
    inverse(&reduction.transform, &reduction.inverse, count, budget)?;
    inverse(&reduction.inverse, &reduction.transform, count, budget)?;
    candidate(cas, selection, reduction, budget)?;
    super::nearest::with_basis(cas, selection, budget, |basis, budget| {
        let bytes = size(slots, AT)?.saturating_mul(
            budget
                .limits()
                .component_bits
                .div_ceil(8)
                .saturating_mul(2)
                .saturating_add(128),
        );
        budget.with_retained_payload(bytes, AT, |budget| {
            let reduced = multiply(basis, &reduction.transform, count, budget)?;
            super::nearest::gram(&reduced, count, budget)
        })
    })
}

/// Compute one exact entry of a finite matrix product.
fn dot(
    left: &[Rational],
    right: &[Rational],
    count: usize,
    row: usize,
    column: usize,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let mut value = Rational::zero();
    for inner in 0..count {
        budget.work(1, AT)?;
        value = value.add(
            &left[row * count + inner].mul(&right[inner * count + column], budget)?,
            budget,
        )?;
    }
    Ok(value)
}

/// Check every entry of one ordered product against the identity.
fn inverse(
    left: &[Rational],
    right: &[Rational],
    count: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for row in 0..count {
        for column in 0..count {
            let expected = if row == column {
                Rational::one()
            } else {
                Rational::zero()
            };
            if dot(left, right, count, row, column, budget)?.compare(&expected, budget)?
                != Ordering::Equal
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Allocate a bounded exact matrix product in row-major order.
fn multiply(
    left: &[Rational],
    right: &[Rational],
    count: usize,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let slots = count.checked_mul(count).ok_or_else(invalid)?;
    budget.check(BudgetKind::MatrixEntries, size(slots, AT)?, AT)?;
    budget.work(size(slots, AT)?, AT)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(slots)
        .map_err(|_| Fault::allocation(AT))?;
    for row in 0..count {
        for column in 0..count {
            values.push(dot(left, right, count, row, column, budget)?);
        }
    }
    Ok(values)
}

/// Match the original proposed tuple to the exact transported reduced tuple.
fn candidate(
    cas: &Cas,
    selection: &LatticeSelection,
    reduction: &LatticeReduction,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let context = &selection.cover.index.basis.context;
    let count = selection.values.len();
    for (row, expected) in selection.values.iter().enumerate() {
        let mut value = Rational::zero();
        for (column, reduced) in reduction.values.iter().enumerate() {
            let reduced = super::literal(cas, reduced, context)?;
            value = value.add(
                &reduction.transform[row * count + column].mul(reduced, budget)?,
                budget,
            )?;
        }
        if value.compare(super::literal(cas, expected, context)?, budget)? != Ordering::Equal {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Each independent tie is oriented by its column's first nonzero original coordinate.
pub(super) fn cuts(
    cas: &Cas,
    selection: &LatticeSelection,
    reduction: &LatticeReduction,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = selection.values.len();
    let half = Rational::from_ratio(1, 2, budget)?;
    for (row, selected) in reduction.values.iter().enumerate() {
        let (lower, upper) = bounds(reduction, row, count, budget)?;
        let value = super::literal(cas, selected, &selection.cover.index.basis.context)?;
        let positive = orientation(&reduction.transform, row, count, budget)?;
        let lower = lower.add(&half, budget)?;
        let upper = upper.add(&half, budget)?;
        if positive {
            super::cell(value, &lower, &upper, budget)?;
        } else {
            let next = value.add(&Rational::one(), budget)?;
            let lower_owned = value.compare(&lower, budget)? == Ordering::Less;
            let upper_owned = upper.compare(&next, budget)? != Ordering::Greater;
            if !lower_owned || !upper_owned {
                return Err(Fault::invalid(
                    InvalidCode::DecisionBoundaryIntersection,
                    super::AT,
                ));
            }
        }
    }
    Ok(())
}

/// The first nonzero original component decides a column's lexicographic sign.
fn orientation(
    transform: &[Rational],
    column: usize,
    count: usize,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for row in 0..count {
        budget.work(1, AT)?;
        let value = &transform[row * count + column];
        if !value.is_zero() {
            return Ok(!value.is_negative());
        }
    }
    Err(invalid())
}

/// Map the complete original coordinate box through one row of W.
fn bounds(
    reduction: &LatticeReduction,
    row: usize,
    count: usize,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    let mut lower = Rational::zero();
    let mut upper = Rational::zero();
    for column in 0..count {
        let coefficient = &reduction.inverse[row * count + column];
        let (start, end) = super::bounds(&reduction.enclosure, column)?;
        let (start, end) = if coefficient.is_negative() {
            (end, start)
        } else {
            (start, end)
        };
        lower = lower.add(&coefficient.mul(start, budget)?, budget)?;
        upper = upper.add(&coefficient.mul(end, budget)?, budget)?;
    }
    Ok((lower, upper))
}
