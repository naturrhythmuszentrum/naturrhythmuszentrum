// SPDX-License-Identifier: AGPL-3.0-only
//! Generation follows the original schedule; it confers no checked authority.

use super::{AT, EnclosureCertificate, EnclosureRequest, Interval, boundary, interval_bytes};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId};
use crate::model::Operator;

/// Produce a bounded postorder transcript for the unchanged source expression.
#[cfg(test)]
pub(super) fn generate(
    cas: &Cas,
    request: &EnclosureRequest,
    scope: Vec<u8>,
    grids: (u32, u32),
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<EnclosureCertificate, Fault> {
    generate_planned(
        cas,
        request,
        scope,
        grids,
        super::planning::Plans::default(),
        budget,
        interrupt,
    )
}

/// Generate with the preflighted analytic schedule; no analytic family is inferred from a name.
pub(super) fn generate_planned(
    cas: &Cas,
    request: &EnclosureRequest,
    scope: Vec<u8>,
    grids: (u32, u32),
    plan: super::planning::Plans,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<EnclosureCertificate, Fault> {
    let (grid, presentation) = grids;
    let mut logarithms = Vec::new();
    let mut exponentials = Vec::new();
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
        let generated = node(
            cas,
            request,
            id,
            (left.map(|i| &records[i]), right.map(|i| &records[i])),
            (grid, plan, records.len()),
            budget,
            interrupt,
        )?;
        if let Some(evidence) = generated.log {
            let size = bytes
                .saturating_add(super::log::bytes(&logarithms))
                .saturating_add(super::exp::bytes(&exponentials))
                .saturating_add(super::log::bytes(std::slice::from_ref(&evidence)));
            append(&mut logarithms, evidence, size, budget)?;
        }
        if let Some(evidence) = generated.exp {
            let size = bytes
                .saturating_add(super::log::bytes(&logarithms))
                .saturating_add(super::exp::bytes(&exponentials))
                .saturating_add(super::exp::bytes(std::slice::from_ref(&evidence)));
            append(&mut exponentials, evidence, size, budget)?;
        }
        let result = generated.bound;
        bytes = bytes.saturating_add(interval_bytes(&result));
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes
                .saturating_add(super::log::bytes(&logarithms))
                .saturating_add(super::exp::bytes(&exponentials)),
            AT,
        )?;
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
            .enclosure_saturation
            == 1
        {
            super::EnclosureLaw::SigmoidCompose
        } else if exponentials.is_empty() {
            law(cas, request, !logarithms.is_empty())?
        } else {
            super::EnclosureLaw::ExpTaylor
        },
        version: 1,
        scope,
        grid,
        presentation,
        records,
        logarithms,
        exponentials,
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

/// Select the implemented family without granting the proposed transcript authority.
fn law(cas: &Cas, request: &EnclosureRequest, logs: bool) -> Result<super::EnclosureLaw, Fault> {
    Ok(if logs {
        super::EnclosureLaw::LogAtanh
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_roots
    {
        super::EnclosureLaw::RootGrid
    } else {
        super::EnclosureLaw::RationalInclusion
    })
}

/// Direct nonpositive source domains refuse; computed boundaries remain refinable.
fn logarithm(
    input: &Interval,
    original: &Operator,
    index: usize,
    plan: super::planning::Plans,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, super::LogEvidence), Fault> {
    if (input.lower().is_zero() || input.lower().is_negative())
        && matches!(original, Operator::Free(_) | Operator::Rational(_))
    {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    super::log::produce(
        input,
        u32::try_from(index).map_err(|_| invalid())?,
        plan.log.ok_or_else(invalid)?,
        budget,
        interrupt,
    )
}

/// One original node's proposal, with at most one finite analytic record.
struct Generated {
    /// Proposed outward bound.
    bound: Interval,
    /// Finite logarithm evidence, when this node is a logarithm.
    log: Option<super::LogEvidence>,
    /// Finite exponential evidence, when this node is an exponential.
    exp: Option<super::ExpEvidence>,
}

/// Produce a single original operation after its operands have been scheduled.
fn node(
    cas: &Cas,
    request: &EnclosureRequest,
    id: LogicalId,
    inputs: (Option<&Interval>, Option<&Interval>),
    schedule: (u32, super::planning::Plans, usize),
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Generated, Fault> {
    let (grid, plan, index) = schedule;
    let (left, right) = inputs;
    let expression = cas.state.expression(id)?;
    let mut log = None;
    let mut exp = None;
    let bound = match &expression.operator {
        Operator::Rational(value) => Interval::singleton(value, budget)?,
        Operator::Free(_) => coordinate(cas, request, id, budget)?.copied(budget)?,
        Operator::NaturalLog => {
            let original = cas.state.expression(expression.operands[0])?;
            let (bound, evidence) = logarithm(
                right.ok_or_else(invalid)?,
                &original.operator,
                index,
                plan,
                budget,
                interrupt,
            )?;
            log = Some(evidence);
            bound
        }
        Operator::Sigmoid => {
            let (bound, evidence) = super::saturating::produce(
                right.ok_or_else(invalid)?,
                u32::try_from(index).map_err(|_| invalid())?,
                plan.exp.ok_or_else(invalid)?,
                budget,
                interrupt,
            )?;
            exp = Some(evidence);
            bound
        }
        Operator::Exponential => {
            let (bound, evidence) = super::exp::produce(
                right.ok_or_else(invalid)?,
                u32::try_from(index).map_err(|_| invalid())?,
                plan.exp.ok_or_else(invalid)?,
                budget,
                interrupt,
            )?;
            exp = Some(evidence);
            bound
        }
        Operator::Neg => {
            let value = right.ok_or_else(invalid)?;
            Interval::new(
                value.upper().negated(budget)?,
                value.lower().negated(budget)?,
                budget,
            )?
            .coarsen(grid, budget)?
        }
        operator => binary(
            operator,
            left.ok_or_else(invalid)?,
            right.ok_or_else(invalid)?,
            grid,
            budget,
        )?,
    };
    Ok(Generated { bound, log, exp })
}

/// Check accumulated staged evidence before reserving or appending a record.
fn append<T>(records: &mut Vec<T>, value: T, bytes: u64, budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    records.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    records.push(value);
    Ok(())
}
