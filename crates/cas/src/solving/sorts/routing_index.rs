// SPDX-License-Identifier: AGPL-3.0-only
//! A prepaid header forest resolves each qualified declaration once.

use super::routing::invalid;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Field, Sort};
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};

/// Accounting site for original declaration indexing.
const AT: Location = Location::new("declared scalar support index");
/// A declaration retains its original introducing header.
type Support = Vec<(Handle, String)>;
/// A canonical header's distinct reached free uses.
type Uses<'a> = HashMap<NodeIndex<u32>, HashMap<&'a str, &'a Sort>>;
/// Current conservative ancestry, without copying inherited environments.
type Active<'a> = HashMap<&'a str, (LogicalId, &'a Sort)>;
/// Direct descendants of each reached conservative header.
type Children = HashMap<NodeIndex<u32>, Vec<NodeIndex<u32>>>;
/// Explicit header enter/exit events.
type Traversal = Vec<(NodeIndex<u32>, bool)>;

/// Resolve a whole header forest with linear scratch and charged canonical output sorting.
pub(super) fn resolve(
    cas: &Cas,
    headers: &HashMap<NodeIndex<u32>, LogicalId>,
    uses: &Uses<'_>,
    retained: usize,
    budget: &mut Budget,
) -> Result<(Support, Support), Fault> {
    scratch(cas, headers, uses, retained, budget)?;
    let (children, mut pending) = forest(cas, headers, budget)?;
    let mut active = Active::new();
    let mut seen = HashSet::new();
    let mut real = Vec::new();
    let mut complex = Vec::new();
    while let Some((index, exiting)) = pending.pop() {
        let id = *headers.get(&index).ok_or_else(invalid)?;
        let header = cas.state.header(id)?;
        budget.work(size(header.declarations().len().saturating_add(1), AT)?, AT)?;
        if exiting {
            for declaration in header.declarations() {
                active.remove(declaration.key.as_str());
            }
            continue;
        }
        insert(&mut active, id, header.declarations(), budget)?;
        if let Some(queries) = uses.get(&index) {
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
        if let Some(descendants) = children.get(&index) {
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
    headers: &HashMap<NodeIndex<u32>, LogicalId>,
    budget: &mut Budget,
) -> Result<(Children, Traversal), Fault> {
    let count = size(headers.len(), AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(192),
        AT,
    )?;
    budget.work(count.saturating_mul(1024), AT)?;
    let mut children: HashMap<_, Vec<_>> = HashMap::new();
    let mut pending = Vec::new();
    children
        .try_reserve(headers.len())
        .map_err(|_| Fault::allocation(AT))?;
    pending
        .try_reserve(headers.len().saturating_mul(2))
        .map_err(|_| Fault::allocation(AT))?;
    for (index, id) in headers {
        if let Some(parent) = cas.state.header(*id)?.parent {
            let parent = *cas.state.indices.get(&parent).ok_or_else(invalid)?;
            let descendants = children.entry(parent).or_default();
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

/// Fresh declarations cannot shadow an ancestor; all key bytes are charged before hashing.
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
    active
        .try_reserve(declarations.len())
        .map_err(|_| Fault::allocation(AT))?;
    for declaration in declarations {
        if active
            .insert(&declaration.key, (header, &declaration.sort))
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
    queries: &HashMap<&str, &Sort>,
    active: &Active<'_>,
    seen: &mut HashSet<(u64, String)>,
    real: &mut Support,
    complex: &mut Support,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (key, claimed) in queries {
        budget.work(size(key.len(), AT)?.saturating_mul(3).saturating_add(1), AT)?;
        let (header, actual) = active.get(key).ok_or_else(invalid)?;
        if actual != claimed {
            return Err(invalid());
        }
        seen.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        if !seen.insert((header.serial, (*key).to_owned())) {
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
    headers: &HashMap<NodeIndex<u32>, LogicalId>,
    uses: &Uses<'_>,
    retained: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut bytes = size(retained, AT)?.saturating_mul(1024);
    for id in headers.values() {
        budget.work(256, AT)?;
        bytes = bytes.saturating_add(
            size(cas.state.header(*id)?.declarations().len(), AT)?.saturating_mul(256),
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
