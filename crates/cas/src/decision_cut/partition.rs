// SPDX-License-Identifier: AGPL-3.0-only
//! Complete finite partitions over exact axis-aligned affine cells.

mod atoms;
mod grid;
mod integer;

use super::{evaluate, plan};
use crate::budget::{Budget, BudgetKind};
use crate::decision::{CaseCut, CasePartition, CheckedCasePartition, CoordinateInterval};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::number::Rational;

/// Stable location for partition admission and sealed-contract reuse.
const AT: Location = Location::new("decision partition admission");

/// Check all mathematical premises before publishing a sealed caller-owned contract.
pub(crate) fn check(
    cas: &Cas,
    contract: &CasePartition,
    budget: &mut Budget,
) -> Result<CheckedCasePartition, Fault> {
    let bytes = payload(contract, budget)?;
    let function = cas.state.expression(cas.checked(&contract.function)?)?;
    let scratch = size(function.arms.len(), AT)?
        .saturating_mul(64)
        .saturating_add(size(contract.coordinates.len(), AT)?.saturating_mul(512));
    budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
        let (cut, header, roots) = prepare(cas, contract, budget)?;
        let order = plan::order(cas, &roots, budget)?;
        let workspace = size(order.len(), AT)?
            .saturating_mul(256)
            .saturating_add(128);
        budget.with_retained_payload(workspace, AT, |budget| {
            reconstruct(cas, contract, &cut, header, &roots, &order, budget)
        })
    })?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
    budget.work(bytes, AT)?;
    Ok(CheckedCasePartition {
        contract: contract.clone(),
        bytes,
    })
}

/// Reserve exact input ownership, including integer endpoint copies, before replay.
fn payload(contract: &CasePartition, budget: &mut Budget) -> Result<u64, Fault> {
    budget.check(
        BudgetKind::ConditionAtoms,
        size(contract.coordinates.len(), AT)?,
        AT,
    )?;
    budget.work(size(contract.coordinates.len(), AT)?.saturating_add(1), AT)?;
    let mut bytes =
        128_u64.saturating_add(size(contract.coordinates.len(), AT)?.saturating_mul(64));
    if let Some(range) = &contract.integer {
        bytes = bytes.saturating_add(64);
        for bound in [range.lower.as_ref(), range.upper.as_ref()]
            .into_iter()
            .flatten()
        {
            bound.check(budget, AT)?;
            bytes = bytes.saturating_add(super::number_bytes(bound));
        }
    }
    Ok(bytes)
}

/// Read the complete actual association inventory; no caller-provided region can replace it.
fn prepare(
    cas: &Cas,
    contract: &CasePartition,
    budget: &mut Budget,
) -> Result<(CaseCut, LogicalId, Vec<LogicalId>), Fault> {
    let function = cas.state.expression(cas.checked(&contract.function)?)?;
    budget.specialized(
        BudgetKind::CaseArms,
        size(function.arms.len(), AT)?,
        false,
        AT,
    )?;
    let selected = function
        .arms
        .first()
        .copied()
        .ok_or_else(|| invalid(InvalidCode::CoverageHole))?;
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(contract.coordinates.len())
        .map_err(|_| Fault::allocation(AT))?;
    for coordinate in &contract.coordinates {
        bounds.push(CoordinateInterval {
            coordinate: coordinate.clone(),
            lower: Rational::zero(),
            upper: Rational::zero(),
        });
    }
    let cut = CaseCut {
        function: contract.function.clone(),
        association: cas.handle(selected),
        bounds,
        integer: None,
    };
    let (header, initial) = super::identities(cas, &cut, budget)?;
    super::input_payload(&cut, budget)?;
    super::validate_box(cas, &cut, header, budget)?;
    super::axes(cas, &cut, header, initial[0], budget)?;
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(function.arms.len().saturating_add(1))
        .map_err(|_| Fault::allocation(AT))?;
    roots.push(initial[0]);
    for id in &function.arms {
        budget.work(1, AT)?;
        let arm = cas.state.arm(*id)?;
        if arm.header != header || !arm.scope.is_empty() {
            return Err(invalid(InvalidCode::Scope));
        }
        if !arm.placements.is_empty() {
            return Err(super::unsupported());
        }
        roots.push(arm.region);
    }
    Ok((cut, header, roots))
}

/// Compile original total affine operands once, then exhaust their exact common cells.
fn reconstruct(
    cas: &Cas,
    contract: &CasePartition,
    cut: &CaseCut,
    header: LogicalId,
    roots: &[LogicalId],
    order: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let values = evaluate::build(cas, cut, header, roots[0], order, budget)?;
    let bytes = evaluate::payload(&values, budget)?;
    let mut program = budget.with_retained_payload(bytes, AT, |budget| {
        atoms::compile(cas, order, &values, cut.bounds.len(), budget)
    })?;
    drop(values);
    let family = integer::prepare(cas, contract, &mut program, budget)?;
    let bytes = program.payload(budget)?.saturating_add(
        size(roots.len(), AT)?
            .saturating_mul(32)
            .saturating_add(128),
    );
    budget.with_retained_payload(bytes, AT, |budget| {
        grid::check(cas, roots, order, &program, family.as_ref(), budget)
    })
}

/// Reuse is a checked association with a sealed contract, never a new cover computation.
pub(crate) fn reuse(
    cas: &Cas,
    partition: &CheckedCasePartition,
    cut: &CaseCut,
    budget: &mut Budget,
) -> Result<(), Fault> {
    cas.checked(&partition.contract.function)?;
    let bytes = super::input_payload(cut, budget)?;
    budget.work(bytes.saturating_add(partition.bytes), AT)?;
    let coordinates = cut.bounds.iter().map(|value| &value.coordinate);
    let matches = partition.contract.function == cut.function
        && partition.contract.coordinates.iter().eq(coordinates)
        && partition.contract.integer == cut.integer;
    if !matches {
        return Err(invalid(InvalidCode::DecisionOptionMismatch));
    }
    budget.with_retained_payload(partition.bytes, AT, |budget| super::check(cas, cut, budget))
}

/// Keep contract refusals distinct from unsupported exact grammar.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}
