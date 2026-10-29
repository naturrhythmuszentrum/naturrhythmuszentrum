// SPDX-License-Identifier: AGPL-3.0-only
//! Exact orthogonal nearest selection shares the original floor enclosure and tuple machinery.

use super::{LatticeEnclosure, LatticeSelection};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, size};
use crate::number::Rational;

/// Only nearest callers pay for Gram replay and its rational arithmetic.
const AT: Location = Location::new("lattice orthogonal nearest");

/// A nonorthogonal or unsupported exact basis cannot borrow coordinate rounding authority.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Replaying every original entry precedes cancellation in the exact positive-diagonal Gram test.
pub(super) fn orthogonal(
    cas: &Cas,
    selection: &LatticeSelection,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let entries = &cas
        .state
        .expression(cas.checked(&selection.cover.index.basis.basis)?)?
        .operands;
    let count = selection.values.len();
    let slots = size(entries.len(), AT)?;
    budget.check(BudgetKind::MatrixEntries, slots, AT)?;
    let bytes = slots.saturating_mul(
        budget
            .limits()
            .component_bits
            .div_ceil(8)
            .saturating_mul(2)
            .saturating_add(128),
    );
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut values = Vec::new();
        budget.work(slots, AT)?;
        values
            .try_reserve_exact(entries.len())
            .map_err(|_| Fault::allocation(AT))?;
        for entry in entries {
            let value = ExactConstant::from_expression(cas, &cas.handle(*entry), budget)?;
            let rational = value.as_rational().ok_or_else(unsupported)?;
            rational.check(budget, AT)?;
            values.push(rational.clone());
        }
        gram(&values, count, budget)
    })
}

/// Every off-diagonal inner product vanishes and every squared column norm is positive.
fn gram(values: &[Rational], count: usize, budget: &mut Budget) -> Result<(), Fault> {
    for column in 0..count {
        for other in column..count {
            let mut product = Rational::zero();
            for row in 0..count {
                budget.work(1, AT)?;
                let term =
                    values[row * count + column].mul(&values[row * count + other], budget)?;
                product = product.add(&term, budget)?;
            }
            let valid = if column == other {
                product.compare(&Rational::zero(), budget)? == std::cmp::Ordering::Greater
            } else {
                product.is_zero()
            };
            if !valid {
                return Err(unsupported());
            }
        }
    }
    Ok(())
}

/// The common half-open floor test on H + 1/2 owns simultaneous ties in original index order.
pub(super) fn cuts(
    cas: &Cas,
    selection: &LatticeSelection,
    enclosure: &LatticeEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let half = Rational::from_ratio(1, 2, budget)?;
    for (coordinate, selected) in selection.values.iter().enumerate() {
        let value = super::literal(cas, selected, &selection.cover.index.basis.context)?;
        let (lower, upper) = super::bounds(enclosure, coordinate)?;
        let lower = lower.add(&half, budget)?;
        let upper = upper.add(&half, budget)?;
        super::cell(value, &lower, &upper, budget)?;
    }
    Ok(())
}
