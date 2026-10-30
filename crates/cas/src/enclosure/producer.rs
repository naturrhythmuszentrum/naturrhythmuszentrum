// SPDX-License-Identifier: AGPL-3.0-only
//! Generation follows the original schedule; it confers no checked authority.

use super::{AT, EnclosureCertificate, EnclosureRequest, Interval, boundary, interval_bytes};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId};
use crate::model::Operator;

/// Produce a bounded postorder transcript for the unchanged source expression.
pub(super) fn generate(
    cas: &Cas,
    request: &EnclosureRequest,
    scope: Vec<u8>,
    grids: (u32, u32),
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<EnclosureCertificate, Fault> {
    let (grid, presentation) = grids;
    let schedule = super::scope::schedule(cas, request, budget)?;
    let mut records: Vec<Interval> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    records
        .try_reserve(schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    stack
        .try_reserve(schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut bytes = u64::try_from(scope.len())
        .unwrap_or(u64::MAX)
        .saturating_add(512);
    for id in schedule {
        boundary(budget, interrupt)?;
        let expression = cas.state.expression(id)?;
        let right = if expression.operands.is_empty() {
            None
        } else {
            Some(stack.pop().ok_or_else(invalid)?)
        };
        let left = if expression.operands.len() == 2 {
            Some(stack.pop().ok_or_else(invalid)?)
        } else {
            None
        };
        let result = match &expression.operator {
            Operator::Rational(value) => Interval::singleton(value, budget)?,
            Operator::Free(_) => coordinate(cas, request, id, budget)?.copied(budget)?,
            Operator::Neg => {
                let value = &records[right.ok_or_else(invalid)?];
                Interval::new(
                    value.upper().negated(budget)?,
                    value.lower().negated(budget)?,
                    budget,
                )?
                .coarsen(grid, budget)?
            }
            operator => binary(
                operator,
                &records[left.ok_or_else(invalid)?],
                &records[right.ok_or_else(invalid)?],
                grid,
                budget,
            )?,
        };
        bytes = bytes.saturating_add(interval_bytes(&result));
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::WitnessBytes, bytes, AT)?;
        budget.work(interval_bytes(&result), AT)?;
        stack.push(records.len());
        records.push(result);
    }
    if stack.len() != 1 {
        return Err(invalid());
    }
    Ok(EnclosureCertificate {
        law: if cas
            .state
            .expression(cas.checked(&request.target)?)?
            .enclosure_roots
        {
            super::EnclosureLaw::RootGrid
        } else {
            super::EnclosureLaw::RationalInclusion
        },
        version: 1,
        scope,
        grid,
        presentation,
        records,
    })
}

/// Find the explicitly supplied bound for an original free coordinate.
fn coordinate<'a>(
    cas: &Cas,
    request: &'a EnclosureRequest,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<&'a Interval, Fault> {
    budget.work(crate::graph::size(request.coordinates.len(), AT)?, AT)?;
    for bound in &request.coordinates {
        if cas.checked(&bound.coordinate)? == id {
            return Ok(&bound.interval);
        }
    }
    Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT))
}

/// Apply the supported interval law before outward rounding.
fn binary(
    operator: &Operator,
    left: &Interval,
    right: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    match operator {
        Operator::GeneralPower(crate::model::Branch::StrictReal) => {
            super::roots::produce(left, grid, budget)
        }
        Operator::Add => left.add(right, grid, budget),
        Operator::Sub => left.sub(right, grid, budget),
        Operator::Mul => left.mul(right, grid, budget),
        Operator::Div => left.mul(&right.reciprocal(grid, budget)?, grid, budget),
        _ => Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
    }
}

/// Refuse a malformed original rational expression.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Encoding, AT)
}
