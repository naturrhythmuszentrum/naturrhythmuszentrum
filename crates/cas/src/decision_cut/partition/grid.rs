// SPDX-License-Identifier: AGPL-3.0-only
//! Exhaustive Cartesian cell ownership, including every singleton and unbounded ray.

use std::collections::BTreeMap;

use super::atoms::{self, Atom, Program};
use super::integer::Family;
use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::model::PredicateKind;

/// Exhaust every nonempty Cartesian cell before accepting any declared option inventory.
pub(super) fn check(
    cas: &Cas,
    roots: &[LogicalId],
    order: &[LogicalId],
    program: &Program,
    family: Option<&Family>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = count(program, budget)?;
    let scratch = size(order.len(), AT)?
        .saturating_mul(256)
        .saturating_add(size(program.cuts.len(), AT)?.saturating_mul(64));
    budget.with_retained_payload(scratch, AT, |budget| {
        let mut position = Vec::new();
        position
            .try_reserve_exact(program.cuts.len())
            .map_err(|_| Fault::allocation(AT))?;
        position.resize(program.cuts.len(), 0);
        let mut inhabited = false;
        for _ in 0..count {
            let truth = evaluate(cas, order, program, &position, budget)?;
            inhabited |= ownership(roots, &truth, family, &position, budget)?;
            advance(program, &mut position, budget)?;
        }
        if inhabited {
            Ok(())
        } else {
            Err(invalid(InvalidCode::EmptyDomain))
        }
    })
}

/// Bound the full Cartesian product before allocating or visiting any of its cells.
fn count(program: &Program, budget: &mut Budget) -> Result<u64, Fault> {
    let mut count = 1_u64;
    for axis in &program.cuts {
        budget.work(1, AT)?;
        let width = size(axis.len(), AT)?
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
        count = count
            .checked_mul(width)
            .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
        budget.specialized(BudgetKind::RegionCells, count, false, AT)?;
    }
    Ok(count)
}

/// Advance the mixed-radix cell position without constructing numeric sample points.
fn advance(program: &Program, position: &mut [usize], budget: &mut Budget) -> Result<(), Fault> {
    for (cell, cuts) in position.iter_mut().zip(&program.cuts) {
        budget.work(1, AT)?;
        *cell = cell
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
        if *cell <= cuts.len().saturating_mul(2) {
            break;
        }
        *cell = 0;
    }
    Ok(())
}

/// Require exactly one owner inside the domain and none outside, with integer cells matched.
fn ownership(
    roots: &[LogicalId],
    truth: &BTreeMap<LogicalId, bool>,
    family: Option<&Family>,
    position: &[usize],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let domain = get(truth, *roots.first().ok_or_else(super::super::unsupported)?)?;
    let mut owners = 0_u64;
    for (index, root) in roots.iter().skip(1).enumerate() {
        budget.work(1, AT)?;
        let present = get(truth, *root)?;
        owners += u64::from(present);
        if let Some(family) = family {
            let cell = position.first().ok_or_else(super::super::unsupported)?;
            let expected = family
                .members
                .get(index)
                .ok_or_else(super::super::unsupported)?;
            if present != expected.contains(cell) || domain != family.domain.contains(cell) {
                return Err(invalid(InvalidCode::DecisionIntegerRange));
            }
        }
    }
    if owners > 1 {
        return Err(invalid(InvalidCode::DecisionPartitionOverlap));
    }
    if domain && owners == 0 {
        return Err(invalid(InvalidCode::CoverageHole));
    }
    if !domain && owners != 0 {
        return Err(invalid(InvalidCode::DecisionPartitionOutsideDomain));
    }
    Ok(domain)
}

/// Evaluate all original total Boolean constructors in their checked dependency order.
fn evaluate(
    cas: &Cas,
    order: &[LogicalId],
    program: &Program,
    position: &[usize],
    budget: &mut Budget,
) -> Result<BTreeMap<LogicalId, bool>, Fault> {
    let mut truth = BTreeMap::new();
    for id in order {
        budget.work(size(truth.len(), AT)?.saturating_add(1), AT)?;
        let value = if let Some(atom) = program.atoms.get(id) {
            atom_truth(atom, program, position, budget)?
        } else if let Vertex::Region(value) = cas.state.node(*id)? {
            region(&value.expression, &truth, budget)?
        } else if let Some((kind, arguments)) = atoms::predicate(cas, *id)? {
            match kind {
                PredicateKind::And => boolean(arguments, &truth, true, budget)?,
                PredicateKind::Or => boolean(arguments, &truth, false, budget)?,
                PredicateKind::Not => !get(
                    &truth,
                    *arguments.first().ok_or_else(super::super::unsupported)?,
                )?,
                _ => return Err(super::super::unsupported()),
            }
        } else {
            continue;
        };
        truth.insert(*id, value);
    }
    Ok(truth)
}

/// Derive uniform cell truth from its order relative to the exact singleton boundary.
fn atom_truth(
    atom: &Atom,
    program: &Program,
    position: &[usize],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match atom {
        Atom::Constant(value) => Ok(*value),
        Atom::Cut {
            axis,
            point,
            negative,
            comparison,
        } => {
            let boundary = program.position(*axis, point, budget)?;
            let mut sign = position
                .get(*axis)
                .ok_or_else(super::super::unsupported)?
                .cmp(&boundary);
            if *negative {
                sign = sign.reverse();
            }
            Ok(atoms::holds(*comparison, sign))
        }
    }
}

/// Read only an earlier reconstructed Boolean value.
fn get(truth: &BTreeMap<LogicalId, bool>, id: LogicalId) -> Result<bool, Fault> {
    truth
        .get(&id)
        .copied()
        .ok_or_else(super::super::unsupported)
}

/// Interpret native set constructors over total cell membership.
fn region(
    value: &RegionExpr<LogicalId>,
    truth: &BTreeMap<LogicalId, bool>,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match value {
        RegionExpr::Ambient => Ok(true),
        RegionExpr::Empty => Ok(false),
        RegionExpr::GuardTrue { base, predicate } => {
            Ok(get(truth, *base)? && get(truth, *predicate)?)
        }
        RegionExpr::GuardFalse { base, predicate } => {
            Ok(get(truth, *base)? && !get(truth, *predicate)?)
        }
        RegionExpr::Meet(members) => boolean(members, truth, true, budget),
        RegionExpr::Union(members) => boolean(members, truth, false, budget),
        RegionExpr::Difference { base, excluded } => {
            Ok(get(truth, *base)? && !get(truth, *excluded)?)
        }
    }
}

/// Consume every ordered Boolean operand and preserve exact conjunction or disjunction.
fn boolean(
    arguments: &[LogicalId],
    truth: &BTreeMap<LogicalId, bool>,
    conjunction: bool,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut result = conjunction;
    for id in arguments {
        budget.work(size(truth.len(), AT)?.saturating_add(1), AT)?;
        let value = get(truth, *id)?;
        result = if conjunction {
            result && value
        } else {
            result || value
        };
    }
    Ok(result)
}
