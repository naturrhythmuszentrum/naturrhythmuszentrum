// SPDX-License-Identifier: AGPL-3.0-only
//! A prepaid header forest resolves each qualified declaration once.

use super::routing::invalid;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Field, Sort};
use crate::ordered_scratch::Map;
use petgraph::graph::NodeIndex;

/// Accounting site for original declaration indexing.
const AT: Location = Location::new("declared scalar support index");
/// A declaration retains its original introducing header.
type Support = Vec<(Handle, String)>;
/// A canonical header's distinct reached free uses.
type Uses<'a> = Map<NodeIndex<u32>, Map<&'a str, &'a Sort>>;
/// Current conservative ancestry, without copying inherited environments.
type Active<'a> = Map<&'a str, (LogicalId, &'a Sort)>;
/// Direct descendants of each reached conservative header.
type Children = Map<NodeIndex<u32>, Vec<NodeIndex<u32>>>;
/// Explicit header enter/exit events.
type Traversal = Vec<(NodeIndex<u32>, bool)>;

/// Resolve a whole header forest with linear scratch and charged canonical output sorting.
pub(super) fn resolve(
    cas: &Cas,
    headers: &Map<NodeIndex<u32>, LogicalId>,
    uses: &Uses<'_>,
    retained: usize,
    budget: &mut Budget,
) -> Result<(Support, Support), Fault> {
    scratch(cas, headers, uses, retained, budget)?;
    let (children, mut pending) = forest(cas, headers, budget)?;
    let mut active = Active::new();
    let mut seen = Map::new();
    let mut real = Vec::new();
    let mut complex = Vec::new();
    while let Some((index, exiting)) = pending.pop() {
        let id = *headers.get(&index, budget, AT)?.ok_or_else(invalid)?;
        let header = cas.state.header(id, budget)?;
        budget.work(size(header.declarations().len().saturating_add(1), AT)?, AT)?;
        if exiting {
            for declaration in header.declarations() {
                active.remove(declaration.key.as_str(), budget, AT)?;
            }
            continue;
        }
        insert(&mut active, id, header.declarations(), budget)?;
        if let Some(queries) = uses.get(&index, budget, AT)? {
            resolve_uses(
                cas,
                queries,
                &active,
                &mut seen,
                &mut real,
                &mut complex,
                budget,
            )?;
        }
        pending.push((index, true));
        if let Some(descendants) = children.get(&index, budget, AT)? {
            pending.extend(descendants.iter().map(|child| (*child, false)));
        }
    }
    canonical(&mut real, budget)?;
    canonical(&mut complex, budget)?;
    Ok((real, complex))
}

/// Preallocate the explicit forest and enter/exit stack before linking its edges.
fn forest(
    cas: &Cas,
    headers: &Map<NodeIndex<u32>, LogicalId>,
    budget: &mut Budget,
) -> Result<(Children, Traversal), Fault> {
    let count = size(headers.len(), AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(192),
        AT,
    )?;
    budget.work(count.saturating_mul(1024), AT)?;
    let mut children: Map<_, Vec<_>> = Map::new();
    let mut pending = Vec::new();
    children.reserve(headers.len(), budget, AT)?;
    pending
        .try_reserve(headers.len().saturating_mul(2))
        .map_err(|_| Fault::allocation(AT))?;
    for (index, id) in headers.iter() {
        if let Some(parent) = cas.state.header(*id, budget)?.parent {
            let parent = cas.state.index(parent, budget)?;
            if children.get(&parent, budget, AT)?.is_none() {
                children.insert(parent, Vec::new(), budget, AT)?;
            }
            let descendants = children.get_mut(&parent, budget, AT)?.ok_or_else(invalid)?;
            descendants
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            descendants.push(*index);
        } else {
            pending.push((*index, false));
        }
    }
    Ok((children, pending))
}

/// Fresh declarations cannot shadow an ancestor; key bytes and ordered comparisons are charged before lookup.
fn insert<'a>(
    active: &mut Active<'a>,
    header: LogicalId,
    declarations: &'a [crate::model::Declaration],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let bytes = declarations.iter().try_fold(0_u64, |sum, value| {
        Ok::<_, Fault>(sum.saturating_add(size(value.key.len(), AT)?))
    })?;
    budget.work(bytes, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(active.len().saturating_add(declarations.len()), AT)?.saturating_mul(128),
        AT,
    )?;
    active.reserve(declarations.len(), budget, AT)?;
    for declaration in declarations {
        if active
            .insert(
                declaration.key.as_str(),
                (header, &declaration.sort),
                budget,
                AT,
            )?
            .is_some()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Only actually reached Free uses enter support; unused declarations supply no route flag.
fn resolve_uses(
    cas: &Cas,
    queries: &Map<&str, &Sort>,
    active: &Active<'_>,
    seen: &mut Map<(u64, String), ()>,
    real: &mut Support,
    complex: &mut Support,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (key, claimed) in queries.iter() {
        budget.work(size(key.len(), AT)?.saturating_mul(3).saturating_add(1), AT)?;
        let (header, actual) = active.get(*key, budget, AT)?.ok_or_else(invalid)?;
        if actual != claimed {
            return Err(invalid());
        }
        if seen
            .insert((header.serial, (*key).to_owned()), (), budget, AT)?
            .is_some()
        {
            continue;
        }
        let destination = match actual {
            Sort::Value(AtomicSort::Scalar(Field::Real)) => &mut *real,
            Sort::Value(AtomicSort::Scalar(Field::Complex)) => &mut *complex,
            _ => continue,
        };
        destination
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        destination.push((cas.handle(*header), (*key).to_owned()));
    }
    Ok(())
}

/// Sorting costs are prepaid from the actual retained key bytes.
fn canonical(values: &mut Support, budget: &mut Budget) -> Result<(), Fault> {
    let count = size(values.len(), AT)?.saturating_add(1);
    let depth = u64::from(count.ilog2()).saturating_add(1);
    let bytes = values.iter().try_fold(0_u64, |sum, (_, key)| {
        Ok::<_, Fault>(sum.saturating_add(size(key.len(), AT)?.saturating_add(16)))
    })?;
    budget.work(bytes.saturating_mul(depth).saturating_mul(2), AT)?;
    values.sort_by(|(left, key), (right, other)| {
        left.logical_id()
            .cmp(&right.logical_id())
            .then_with(|| key.cmp(other))
    });
    Ok(())
}

/// Bound combined live graph, header, declaration and owned-key scratch before building the index.
fn scratch(
    cas: &Cas,
    headers: &Map<NodeIndex<u32>, LogicalId>,
    uses: &Uses<'_>,
    retained: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut bytes = size(retained, AT)?.saturating_mul(1024);
    for id in headers.values() {
        budget.work(256, AT)?;
        bytes = bytes.saturating_add(
            size(cas.state.header(*id, budget)?.declarations().len(), AT)?.saturating_mul(256),
        );
    }
    for queries in uses.values() {
        for key in queries.keys() {
            let length = size(key.len(), AT)?;
            budget.work(length.saturating_add(1), AT)?;
            bytes = bytes.saturating_add(length.saturating_mul(2));
        }
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)
}
