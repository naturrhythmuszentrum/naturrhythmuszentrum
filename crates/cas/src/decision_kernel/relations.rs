// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact rational reductions of every scalar endpoint relation.

use std::cmp::Ordering;

use super::model::{ScalarKernelLaw, ScalarKernelRecord};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::number::Rational;

/// Forged endpoint relations name the exact checker that observed them.
const AT: Location = Location::new("scalar kernel endpoint relations");

/// False interval order or containment has no enclosure authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::DecisionBox, AT)
}

/// Authenticate a backwards reference before indexing an earlier operand box.
pub(super) fn operand(
    records: &[ScalarKernelRecord],
    index: u32,
) -> Result<&ScalarKernelRecord, Fault> {
    records
        .get(usize::try_from(index).map_err(|_| invalid())?)
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

/// Replay both inclusive rational inequalities without any floating conversion.
pub(super) fn contains(
    record: &ScalarKernelRecord,
    value: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let above = record.lower.compare(value, budget)? != Ordering::Greater;
    let below = value.compare(&record.upper, budget)? != Ordering::Greater;
    if above && below {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Replay all domain and endpoint obligations for one complete scalar record.
pub(super) fn check(
    cas: &crate::graph::Cas,
    original: crate::graph::LogicalId,
    record: &ScalarKernelRecord,
    earlier: &[ScalarKernelRecord],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    record.lower.check(budget, AT)?;
    record.upper.check(budget, AT)?;
    if record.lower.compare(&record.upper, budget)? == Ordering::Greater {
        return Err(invalid());
    }
    match &record.law {
        ScalarKernelLaw::Input(value) => super::input::check(cas, original, record, value, budget),
        ScalarKernelLaw::Family(value) => super::family::input(record, value, budget),
        ScalarKernelLaw::Reading(value) => super::reading::enclose(record, value, earlier, budget),
        ScalarKernelLaw::Rational(value) => {
            value.check(budget, AT)?;
            contains(record, value, budget)
        }
        ScalarKernelLaw::Sum { left, right } => {
            let a = operand(earlier, *left)?;
            let b = operand(earlier, *right)?;
            contains(record, &a.lower.add(&b.lower, budget)?, budget)?;
            contains(record, &a.upper.add(&b.upper, budget)?, budget)
        }
        ScalarKernelLaw::Product { left, right } => {
            let a = operand(earlier, *left)?;
            let b = operand(earlier, *right)?;
            for first in [&a.lower, &a.upper] {
                for second in [&b.lower, &b.upper] {
                    contains(record, &first.mul(second, budget)?, budget)?;
                }
            }
            Ok(())
        }
        ScalarKernelLaw::Reciprocal { input } => {
            let a = operand(earlier, *input)?;
            let positive = !a.lower.is_negative() && !a.lower.is_zero();
            if !(positive || a.upper.is_negative()) {
                return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
            }
            contains(record, &Rational::one().div(&a.upper, budget)?, budget)?;
            contains(record, &Rational::one().div(&a.lower, budget)?, budget)
        }
        ScalarKernelLaw::SquareRoot { input } => {
            let a = operand(earlier, *input)?;
            let nonpositive = a.lower.is_negative() || a.lower.is_zero();
            if nonpositive || record.lower.is_negative() {
                return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
            }
            let lower = record.lower.mul(&record.lower, budget)?;
            let upper = record.upper.mul(&record.upper, budget)?;
            let above = lower.compare(&a.lower, budget)? != Ordering::Greater;
            let below = a.upper.compare(&upper, budget)? != Ordering::Greater;
            if above && below {
                Ok(())
            } else {
                Err(invalid())
            }
        }
    }
}
