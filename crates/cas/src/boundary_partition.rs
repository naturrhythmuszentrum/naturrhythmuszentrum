// SPDX-License-Identifier: AGPL-3.0-only
//! Complete finite case-selector partitions reconstructed from the entire function graph.

use crate::boundary_model::{BoundaryCertificate, BoundaryPartition};
use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::{self, invalid};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Partition resource accounting location.
const AT: Location = Location::new("complete boundary partition");

/// Inspect all selector dependencies; no supplied list can omit a reachable association.
pub(crate) fn inventory(
    cas: &mut Cas,
    function: &Handle,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let order = case_syntax::order(cas, &[cas.checked(function)?], budget)?;
    let mut regions = vec![cas.checked(domain)?];
    for id in order {
        crate::named_case::check(cas, id, budget)?;
        let value = cas.state.expression(id)?;
        if let Some(region) = value.domain {
            budget.check(
                BudgetKind::RegionNodes,
                size(
                    regions
                        .len()
                        .saturating_add(value.arms.len())
                        .saturating_add(1),
                    AT,
                )?,
                AT,
            )?;
            regions
                .try_reserve(value.arms.len().saturating_add(1))
                .map_err(|_| Fault::allocation(AT))?;
            regions.push(region);
            for arm in &value.arms {
                regions.push(cas.state.arm(*arm)?.region);
            }
        }
    }
    budget.check(BudgetKind::RegionNodes, size(regions.len(), AT)?, AT)?;
    Ok(case_syntax::canonical(cas, &regions, budget)?
        .into_iter()
        .map(|id| cas.handle(id))
        .collect())
}

/// Construct a proposed complete partition, including empty cells outside the requested region.
pub(crate) fn build(
    cas: &mut Cas,
    function: &Handle,
    domain: &Handle,
    coordinate: &str,
    budget: &mut Budget,
) -> Result<(BoundaryPartition, BoundaryCertificate), Fault> {
    crate::boundary_roles::region(cas, function, domain, coordinate)?;
    let regions = inventory(cas, function, domain, budget)?;
    let roots: Vec<_> = regions.iter().map(Handle::logical_id).collect();
    let partition = crate::region_cache::partition(cas, &roots, budget).map_err(|mut fault| {
        if crate::case_obligations::unknown(&fault) {
            fault.kind =
                crate::error::FaultKind::Invalid(crate::error::InvalidCode::UnknownTotality);
        }
        fault
    })?;
    budget.specialized(
        BudgetKind::BoundaryNodes,
        size(partition.cuts.len(), AT)?,
        false,
        AT,
    )?;
    let mut boundaries = Vec::new();
    boundaries
        .try_reserve_exact(partition.cuts.len())
        .map_err(|_| Fault::allocation(AT))?;
    for cut in &partition.cuts {
        boundaries.push(crate::boundary_regions::boundary(cas, domain, cut, budget)?);
    }
    let count = partition
        .cuts
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
    budget.specialized(BudgetKind::RegionCells, size(count, AT)?, false, AT)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for index in 0..count {
        cells.push(crate::boundary_regions::cell(
            cas,
            domain,
            coordinate,
            &partition.cuts,
            index,
            budget,
        )?);
    }
    Ok((
        BoundaryPartition {
            function: function.clone(),
            domain: domain.clone(),
            coordinate: coordinate.into(),
            boundaries,
            cells,
        },
        BoundaryCertificate::Partition { regions, partition },
    ))
}

/// Independently reconstruct both geometry and certificate before accepting a completeness claim.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !premises.is_empty() {
        return Err(invalid());
    }
    let Statement::BoundaryPartition(claim) = &draft.conclusion else {
        return Err(invalid());
    };
    let within = cas.state.context(cas.checked(&draft.context)?)?.admissible;
    crate::case_check::subset(cas, cas.checked(&claim.domain)?, within, budget)?;
    let (expected, certificate) = build(
        cas,
        &claim.function,
        &claim.domain,
        &claim.coordinate,
        budget,
    )?;
    if **claim != expected || draft.certificate != Certificate::Boundary(Box::new(certificate)) {
        return Err(invalid());
    }
    Ok(())
}
