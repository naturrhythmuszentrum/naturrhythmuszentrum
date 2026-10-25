// SPDX-License-Identifier: AGPL-3.0-only
//! Finite integral option declarations are checked against their exact unit cells.

use std::cmp::Ordering;
use std::ops::Range;

use super::atoms::Program;
use super::{AT, invalid};
use crate::budget::Budget;
use crate::decision::{CasePartition, IntegerCellRange};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::number::Rational;

/// Exact cell-index ranges for the finite declared integer domain and every option.
pub(super) struct Family {
    /// Included lower and excluded upper singleton positions of the declared range.
    pub(super) domain: Range<usize>,
    /// Unit cells in the actual native association order.
    pub(super) members: Vec<Range<usize>>,
}

/// Add every declared integer boundary, then bind each literal option to its unit cell.
pub(super) fn prepare(
    cas: &Cas,
    contract: &CasePartition,
    program: &mut Program,
    budget: &mut Budget,
) -> Result<Option<Family>, Fault> {
    let Some(range) = &contract.integer else {
        return Ok(None);
    };
    let function = cas.state.expression(cas.checked(&contract.function)?)?;
    let scratch = super::super::temporary(0, budget)?
        .saturating_add(size(function.arms.len(), AT)?.saturating_mul(32));
    budget.with_retained_payload(scratch, AT, |budget| {
        let retained = program.payload(budget)?;
        let (lower, upper) = budget
            .with_retained_payload(retained, AT, |budget| endpoints(contract, range, budget))?;
        program.insert(0, lower, budget)?;
        program.insert(0, upper, budget)?;
        for id in &function.arms {
            budget.work(1, AT)?;
            let retained = program.payload(budget)?;
            budget.with_retained_payload(retained, AT, |budget| {
                let index = option(cas, *id)?;
                super::super::check_range(index, range, budget)?;
                program.insert(0, index, budget)?;
                program.insert(0, &index.add(&Rational::one(), budget)?, budget)
            })?;
        }
        let retained = program.payload(budget)?;
        budget.with_retained_payload(retained, AT, |budget| {
            positions(cas, &function.arms, program, lower, upper, budget).map(Some)
        })
    })
}

/// Retain the complete cut program while computing endpoint indices and unit-cell successors.
fn positions(
    cas: &Cas,
    arms: &[LogicalId],
    program: &Program,
    lower: &Rational,
    upper: &Rational,
    budget: &mut Budget,
) -> Result<Family, Fault> {
    let mut members = Vec::new();
    members
        .try_reserve_exact(arms.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in arms {
        let index = option(cas, *id)?;
        let end = index.add(&Rational::one(), budget)?;
        members.push(program.position(0, index, budget)?..program.position(0, &end, budget)?);
    }
    Ok(Family {
        domain: program.position(0, lower, budget)?..program.position(0, upper, budget)?,
        members,
    })
}

/// Finite nonempty ranges have integral endpoints and exactly one matching Real coordinate.
fn endpoints<'a>(
    contract: &CasePartition,
    range: &'a IntegerCellRange,
    budget: &mut Budget,
) -> Result<(&'a Rational, &'a Rational), Fault> {
    if contract.coordinates.len() != 1 || contract.coordinates.first() != Some(&range.coordinate) {
        return Err(invalid(InvalidCode::DecisionIntegerRange));
    }
    let lower = range
        .lower
        .as_ref()
        .ok_or_else(|| invalid(InvalidCode::DecisionIntegerRange))?;
    let upper = range
        .upper
        .as_ref()
        .ok_or_else(|| invalid(InvalidCode::DecisionIntegerRange))?;
    let integral = lower.is_integer() && upper.is_integer();
    if !integral || lower.compare(upper, budget)? != Ordering::Less {
        return Err(invalid(InvalidCode::DecisionIntegerRange));
    }
    Ok((lower, upper))
}

/// Accept only the original association's exactly integral Real rational literal.
fn option(cas: &Cas, association: LogicalId) -> Result<&Rational, Fault> {
    let value = cas.state.expression(cas.state.arm(association)?.value)?;
    let Operator::Rational(index) = &value.operator else {
        return Err(invalid(InvalidCode::DecisionIntegerRange));
    };
    if value.sort != Sort::Value(AtomicSort::Scalar(Field::Real)) || !index.is_integer() {
        return Err(invalid(InvalidCode::DecisionIntegerRange));
    }
    Ok(index)
}
