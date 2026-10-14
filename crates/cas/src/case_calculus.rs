// SPDX-License-Identifier: AGPL-3.0-only
//! Derivative transport on open cells and complete assembly with witnessed point values.

use crate::boundary_check::{equality, premise};
use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};

/// A higher derivative consumes the actual previous derivative, including its owned points.
pub(crate) fn current<'a>(
    function: &'a Handle,
    coordinate: &str,
    order: u32,
    premises: &'a [CheckedApplication],
) -> Result<&'a Handle, Fault> {
    if order == 1 {
        return Ok(function);
    }
    let Statement::CaseDerivative(prefix) = premise(premises, 0)? else {
        return Err(invalid());
    };
    if order == 0
        || prefix.order != order - 1
        || prefix.function != *function
        || prefix.coordinate != coordinate
    {
        return Err(invalid());
    }
    Ok(&prefix.derivative)
}

/// Local previous derivatives suffice only where their established domain contains the whole germ.
pub(crate) fn prefix_domain(
    cas: &Cas,
    premises: &[CheckedApplication],
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::CaseDerivative(prefix) = premise(premises, 0)? else {
        return Err(invalid());
    };
    crate::case_check::subset(cas, region.logical_id(), prefix.domain.logical_id(), budget)
}

/// Transport a first branch derivative through total local equality on an open interval.
pub(crate) fn interior(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::CaseDerivative(claim) = &draft.conclusion else {
        return Err(invalid());
    };
    let offset = usize::from(claim.order > 1);
    if premises.len() != offset + 2 {
        return Err(invalid());
    }
    let source = current(&claim.function, &claim.coordinate, claim.order, premises)?;
    crate::region_cache::open_interval(cas, claim.domain.logical_id(), budget)?;
    if claim.order > 1 {
        prefix_domain(cas, premises, &claim.domain, budget)?;
    }
    let Statement::DerivativeOn {
        function,
        derivative,
        coordinate,
        domain,
    } = premise(premises, offset + 1)?
    else {
        return Err(invalid());
    };
    if derivative != &claim.derivative || coordinate != &claim.coordinate {
        return Err(invalid());
    }
    crate::case_check::subset(cas, claim.domain.logical_id(), domain.logical_id(), budget)?;
    equality(premise(premises, offset)?, source, function, &claim.domain)
}

/// Assemble only a complete requested-domain derivative with one checked fact per inhabited cell.
pub(crate) fn assemble(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::CaseDerivative(claim) = &draft.conclusion else {
        return Err(invalid());
    };
    let Statement::BoundaryPartition(partition) = premise(premises, 0)? else {
        return Err(invalid());
    };
    if partition.function != claim.function || partition.coordinate != claim.coordinate {
        return Err(invalid());
    }
    crate::case_check::subset(
        cas,
        claim.domain.logical_id(),
        partition.domain.logical_id(),
        budget,
    )?;
    let mut consumed = 1;
    for (index, region) in partition.cells.iter().enumerate() {
        if !included(cas, region, &claim.domain, budget)? {
            continue;
        }
        let fact = premise(premises, consumed)?;
        let value = cell_value(fact, claim, partition, region, index)?;
        let analysis = crate::case_cells::analyze(
            cas,
            claim.derivative.logical_id(),
            value.logical_id(),
            region.logical_id(),
            budget,
        )?;
        analysis.check(
            cas,
            claim.derivative.logical_id(),
            value.logical_id(),
            region.logical_id(),
            crate::proof_model::DomainContract::DefinedAndEqual,
            budget,
        )?;
        consumed += 1;
    }
    if consumed != premises.len() {
        return Err(invalid());
    }
    Ok(())
}

/// Each singleton is certified at the original function's actual point, never just branchwise.
fn cell_value<'a>(
    statement: &'a Statement,
    claim: &crate::boundary_model::CaseDerivative,
    partition: &crate::boundary_model::BoundaryPartition,
    region: &Handle,
    index: usize,
) -> Result<&'a Handle, Fault> {
    match statement {
        Statement::CaseDerivative(value)
            if index & 1 == 0
                && value.function == claim.function
                && value.coordinate == claim.coordinate
                && value.order == claim.order
                && value.domain == *region =>
        {
            Ok(&value.derivative)
        }
        Statement::DerivativeAt(value)
            if index & 1 == 1
                && value.function == claim.function
                && value.coordinate == claim.coordinate
                && value.order == claim.order
                && partition.boundaries.get(index / 2) == Some(&value.boundary) =>
        {
            Ok(&value.value)
        }
        _ => Err(invalid()),
    }
}

/// Assembly admits a union of whole certified cells, preserving all endpoint distinctions.
fn included(cas: &Cas, cell: &Handle, domain: &Handle, budget: &mut Budget) -> Result<bool, Fault> {
    let partition =
        crate::region_cache::partition(cas, &[cell.logical_id(), domain.logical_id()], budget)?;
    let first = partition.membership.first().ok_or_else(invalid)?;
    let second = partition.membership.get(1).ok_or_else(invalid)?;
    let any = first
        .iter()
        .zip(second)
        .any(|(left, right)| *left && *right);
    if any
        && !first
            .iter()
            .zip(second)
            .all(|(left, right)| !left || *right)
    {
        return Err(invalid());
    }
    Ok(any)
}
