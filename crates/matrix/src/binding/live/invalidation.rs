// SPDX-License-Identifier: AGPL-3.0-only
//! Map actual writes to declared families and query every retained reading and proof inventory.

use super::{Group, LiveBindingError, admission::charge};
use crate::heap::identity::Key;
use crate::{Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, FaultKind, Handle,
        readings::{DependencyInventory, EntryIndex},
    },
};
use std::rc::Rc;

/// All validity changes remain private until numerical publication succeeds.
pub(crate) type Invalidations = Vec<(Rc<Group>, Vec<usize>)>;

/// A complete authenticated write on one exact mathematical base declaration.
struct GraphWrite {
    /// Graph authority shared by every potentially dependent reading.
    graph: Rc<Cas>,
    /// Exact original free-coordinate symbol.
    base: Handle,
    /// Injective changed coordinates in the declared rectangle.
    entries: Vec<EntryIndex>,
}

/// Unsupported storage-to-family maps grant no fine disjointness claim.
struct Writes {
    /// Every available complete numerical-to-symbolic write map.
    exact: Vec<GraphWrite>,
    /// Graphs whose changed represented family requires coarse invalidation.
    coarse: Vec<Rc<Cas>>,
}

/// Prepare every dependent reader and witness invalidation under the same exclusive lease.
pub(crate) fn prepare(
    heap: &Heap,
    writes: &[(Key, Vec<usize>)],
    budget: &mut Budget,
) -> Result<Invalidations, LiveBindingError> {
    let groups = heap.symbols.groups();
    let mapped = map_writes(&groups, writes, budget)?;
    let mut result = Vec::new();
    for group in groups {
        let affected = affected(&group, &mapped, budget)?;
        result.push((group, affected));
    }
    Ok(result)
}

/// Coarse numerical replacement invalidates safely even if its exact query cannot be completed.
pub(crate) fn whole(heap: &Heap, key: Key) -> Invalidations {
    let Ok(cover) = heap.symbols.cover(key) else {
        return Vec::new();
    };
    let writes = vec![(key, (0..cover.versions.len()).collect())];
    let result = crate::binding::symbols::budget()
        .map_err(LiveBindingError::from)
        .and_then(|mut budget| prepare(heap, &writes, &mut budget));
    result.unwrap_or_else(|_| {
        heap.symbols
            .groups()
            .into_iter()
            .map(|group| {
                let indices = (0..group.states.len()).collect();
                (group, indices)
            })
            .collect()
    })
}

/// Map the whole declared rectangle, including coordinates read only by domain predicates.
fn map_writes(
    groups: &[Rc<Group>],
    writes: &[(Key, Vec<usize>)],
    budget: &mut Budget,
) -> Result<Writes, LiveBindingError> {
    let mut result = Writes {
        exact: Vec::new(),
        coarse: Vec::new(),
    };
    for group in groups {
        for adapter in group.coverage.readings() {
            for entry in adapter.entries() {
                let ticket = group
                    .tickets
                    .get(entry.region)
                    .ok_or(HeapError::InvalidLayout)?;
                let Some((_, changed)) = writes.iter().find(|(key, _)| *key == ticket.key) else {
                    continue;
                };
                let family = adapter.reading().family();
                if !direct(group, adapter, entry.region)? {
                    if !result
                        .coarse
                        .iter()
                        .any(|graph| Rc::ptr_eq(graph, &group.graph))
                    {
                        result.coarse.push(Rc::clone(&group.graph));
                    }
                    continue;
                }
                let entries = coordinates(changed, family.shape()[0], budget)?;
                if !result.exact.iter().any(|write| {
                    Rc::ptr_eq(&write.graph, &group.graph)
                        && write.base == *family.root()
                        && write.entries == entries
                }) {
                    result.exact.push(GraphWrite {
                        graph: Rc::clone(&group.graph),
                        base: family.root().clone(),
                        entries,
                    });
                }
            }
        }
    }
    Ok(result)
}

/// Fine survival requires a complete direct base map and every selected entry in its declared order.
fn direct(
    group: &Group,
    adapter: &crate::ReadingAdapter<usize>,
    region: usize,
) -> Result<bool, HeapError> {
    let family = adapter.reading().family();
    let ticket = group.tickets.get(region).ok_or(HeapError::InvalidLayout)?;
    if family.node() != family.root()
        || family.shape() != crate::binding::symbols::shape(ticket.layout.extent())?
    {
        return Ok(false);
    }
    Ok(adapter
        .entries()
        .iter()
        .zip(adapter.reading().selection().indices())
        .all(|(entry, selected)| {
            entry.region == region
                && entry.index
                    == u64::from(selected.column) * u64::from(family.shape()[0])
                        + u64::from(selected.row)
        }))
}

/// Budget each write map once; no reading or proof dependency extraction occurs here.
fn coordinates(
    changed: &[usize],
    rows: u32,
    budget: &mut Budget,
) -> Result<Vec<EntryIndex>, LiveBindingError> {
    charge::<EntryIndex>(changed.len(), budget)?;
    if rows == 0 && !changed.is_empty() {
        return Err(HeapError::InvalidLayout.into());
    }
    changed
        .iter()
        .map(|&index| {
            let index = u64::try_from(index).map_err(|_| HeapError::ExtentOverflow)?;
            Ok(EntryIndex {
                row: u32::try_from(index % u64::from(rows))
                    .map_err(|_| HeapError::ExtentOverflow)?,
                column: u32::try_from(index / u64::from(rows))
                    .map_err(|_| HeapError::ExtentOverflow)?,
            })
        })
        .collect()
}

/// Every cached witness is queried independently, including its retained domain predicates.
fn affected(
    group: &Group,
    writes: &Writes,
    budget: &mut Budget,
) -> Result<Vec<usize>, LiveBindingError> {
    charge::<bool>(group.states.len(), budget)?;
    if writes
        .coarse
        .iter()
        .any(|graph| Rc::ptr_eq(graph, &group.graph))
    {
        return Ok((0..group.states.len()).collect());
    }
    let mut touched = vec![false; group.states.len()];
    for write in writes
        .exact
        .iter()
        .filter(|write| Rc::ptr_eq(&write.graph, &group.graph))
    {
        let Some(reads) = &group.reads else {
            return Ok((0..group.states.len()).collect());
        };
        for (index, inventory) in reads.iter().enumerate() {
            touched[index] |= intersects(group, inventory, write, budget)?;
        }
        for (companion, inventory) in group.companions.iter().zip(&group.companion_reads) {
            if intersects(group, inventory, write, budget)? {
                for (index, adapter) in group.coverage.readings().iter().enumerate() {
                    if companion
                        .targets()
                        .iter()
                        .any(|target| target.reading() == adapter.reading())
                    {
                        touched[index] = true;
                    }
                }
            }
        }
        for proof in &group.proofs {
            if intersects(group, &proof.inventory, write, budget)? {
                for &index in &proof.readers {
                    touched[index] = true;
                }
            }
        }
    }
    Ok(touched
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| value.then_some(index))
        .collect())
}

/// Invalid fine evidence is conservatively intersecting; resource exhaustion refuses the whole write.
fn intersects(
    group: &Group,
    inventory: &DependencyInventory,
    write: &GraphWrite,
    budget: &mut Budget,
) -> Result<bool, LiveBindingError> {
    match group
        .graph
        .dependency_entry_intersection(inventory, &write.base, &write.entries, budget)
    {
        Ok(touched) => Ok(!touched.is_empty()),
        Err(cause) if matches!(cause.kind, FaultKind::Invalid(_)) => Ok(true),
        Err(cause) => Err(cause.into()),
    }
}

/// Apply a prepared invalidation closure after all numerical records are committed.
pub(crate) fn publish(prepared: Invalidations) {
    for (group, indices) in prepared {
        for index in indices {
            group.states[index].set(crate::SymbolState::Unavailable);
        }
    }
}
