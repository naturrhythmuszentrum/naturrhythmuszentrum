// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical Kahn ordering of the reachable typed witness dependency DAG.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::Layer;
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::proof_model::{GuardedObligation, Judgment};
use crate::rule::Primitive;
use crate::wire_read::invalid;

/// Canonical DAG resource location.
const AT: Location = Location::new("typed witness DAG");

/// Local identities are used only for dependency lookup, never for wire ordering.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Key {
    /// A semantic object or proof application.
    Object(LogicalId),
    /// Complete local guarded tuple.
    Guard(Vec<u8>),
    /// Complete statement and context, excluding residuals.
    Statement(Vec<u8>),
    /// A theorem with its reconstructed conditions.
    Conclusion(Vec<u8>),
}

/// Reachable graph objects and synthetic theorem records.
#[derive(Clone, Debug)]
pub(crate) enum Entity {
    /// An admitted syntax/context object, or a proof naming an application.
    Object(LogicalId),
    /// A guarded predicate including its home and precise domain.
    Guard(GuardedObligation),
    /// A typed statement with its context and mathematical layer.
    Statement(Judgment),
    /// A final theorem; absent proof is used only for portable expected-judgment keys.
    Conclusion {
        /// Checked application root, absent only for expected-judgment keys.
        proof: Option<LogicalId>,
        /// Complete typed theorem and its residual obligations.
        judgment: Judgment,
        /// Qualified assumptions used by the proof.
        hypotheses: Vec<crate::domain_model::HypothesisRef>,
    },
}

/// Measured symbolic record; reference byte positions are patched after dependencies are ready.
#[derive(Debug)]
pub(crate) struct Record {
    /// Wire kind independent of graph kind.
    pub(crate) tag: u8,
    /// Mathematical layer.
    pub(crate) layer: Layer,
    /// Existing exact distribution/test space, when applicable.
    pub(crate) space: Option<Entity>,
    /// Payload after the common prefix, with zero reference placeholders.
    pub(crate) payload: Vec<u8>,
    /// Every strictly backward dependency and its payload position.
    pub(crate) references: Vec<(usize, Entity)>,
    /// Reference ranges whose members are mathematically unordered.
    pub(crate) sets: Vec<Vec<usize>>,
    /// Certificate rows aligned with unordered cover members.
    pub(crate) row_sets: Vec<(usize, usize, Vec<Entity>)>,
    /// An application contributes its exact primitive to the manifest.
    pub(crate) rule: Option<Primitive>,
}

/// Construct bounded local lookup keys without using them as canonical order keys.
pub(crate) fn key(cas: &Cas, value: &Entity, budget: &mut Budget) -> Result<Key, Fault> {
    let mut out = Encoder::new(budget, AT);
    Ok(match value {
        Entity::Object(id) => Key::Object(*id),
        Entity::Guard(value) => {
            crate::statement::guard(value, &mut out, crate::statement::handle)?;
            Key::Guard(out.finish())
        }
        Entity::Statement(value) => {
            crate::graph::encode_id(cas.checked(&value.context)?, &mut out)?;
            out.byte(value.layer.tag())?;
            out.byte(u8::from(value.space.is_some()))?;
            if let Some(space) = &value.space {
                crate::graph::encode_id(cas.checked(space)?, &mut out)?;
            }
            crate::statement::encode(&value.statement, &mut out, crate::statement::handle)?;
            Key::Statement(out.finish())
        }
        Entity::Conclusion {
            proof,
            judgment,
            hypotheses,
        } => {
            out.byte(u8::from(proof.is_some()))?;
            if let Some(proof) = proof {
                crate::graph::encode_id(*proof, &mut out)?;
            }
            crate::statement::judgment(judgment, &mut out)?;
            for reference in hypotheses {
                crate::statement::handle(&reference.context, &mut out)?;
                out.unsigned(u64::from(reference.slot))?;
            }
            Key::Conclusion(out.finish())
        }
    })
}

/// Discover dependencies with an explicit bounded worklist.
fn gather(cas: &Cas, root: Entity, budget: &mut Budget) -> Result<BTreeMap<Key, Record>, Fault> {
    let mut records = BTreeMap::new();
    let mut pending = vec![root];
    let mut retained = 0_u64;
    while let Some(entity) = pending.pop() {
        let id = key(cas, &entity, budget)?;
        if records.contains_key(&id) {
            continue;
        }
        budget.check(
            BudgetKind::Vertices,
            size(records.len(), AT)?
                .checked_add(1)
                .ok_or_else(invalid)?,
            AT,
        )?;
        let record = crate::wire_records::describe(cas, &entity, budget)?;
        let footprint = size(record.payload.len(), AT)?
            .checked_mul(16)
            .and_then(|value| {
                value.checked_add(size(record.references.len(), AT).ok()?.checked_mul(256)?)
            })
            .ok_or_else(invalid)?;
        retained = retained.checked_add(footprint).ok_or_else(invalid)?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.work(footprint, AT)?;
        let count = record
            .references
            .len()
            .checked_add(usize::from(record.space.is_some()))
            .ok_or_else(invalid)?;
        pending
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for (_, child) in &record.references {
            pending.push(child.clone());
        }
        if let Some(space) = &record.space {
            pending.push(space.clone());
        }
        for (_, _, members) in &record.row_sets {
            pending
                .try_reserve(members.len())
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend(members.iter().cloned());
        }
        records.insert(id, record);
    }
    Ok(records)
}

/// Patch one ready descriptor using assigned child indices and structural set-member order.
fn ready(
    cas: &Cas,
    record: &Record,
    assigned: &BTreeMap<Key, (u32, Vec<u8>)>,
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, Fault> {
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(record.references.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (_, entity) in &record.references {
        let id = key(cas, entity, budget)?;
        let Some(value) = assigned.get(&id) else {
            return Ok(None);
        };
        indices.push(value);
    }
    let space = if let Some(space) = &record.space {
        let Some(value) = assigned.get(&key(cas, space, budget)?) else {
            return Ok(None);
        };
        Some(value.0)
    } else {
        None
    };
    budget.work(size(record.payload.len(), AT)?, AT)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(record.payload.len())
        .map_err(|_| Fault::allocation(AT))?;
    payload.extend_from_slice(&record.payload);
    for positions in &record.sets {
        let mut values = Vec::new();
        values
            .try_reserve_exact(positions.len())
            .map_err(|_| Fault::allocation(AT))?;
        for position in positions {
            values.push(*indices.get(*position).ok_or_else(invalid)?);
        }
        budget.work(
            size(values.len(), AT)?
                .checked_mul(size(payload.len(), AT)?)
                .ok_or_else(invalid)?,
            AT,
        )?;
        values.sort_by(|left, right| left.1.cmp(&right.1));
        for (position, value) in positions.iter().zip(values) {
            *indices.get_mut(*position).ok_or_else(invalid)? = value;
        }
    }
    for ((offset, _), (index, _)) in record.references.iter().zip(indices) {
        let end = offset.checked_add(4).ok_or_else(invalid)?;
        payload
            .get_mut(*offset..end)
            .ok_or_else(invalid)?
            .copy_from_slice(&index.to_le_bytes());
    }
    reorder_rows(cas, record, assigned, &mut payload, budget)?;
    let mut out = Encoder::witness(budget, AT);
    out.byte(record.tag)?;
    let length = payload
        .len()
        .checked_add(if space.is_some() { 10 } else { 6 })
        .ok_or_else(invalid)?;
    out.count(length)?;
    crate::wire::index(1, &mut out)?;
    out.byte(record.layer.tag())?;
    out.byte(u8::from(space.is_some()))?;
    if let Some(space) = space {
        crate::wire::index(space, &mut out)?;
    }
    out.raw(&payload)?;
    Ok(Some(out.finish()))
}

/// Encoded records, root index and the exact set of referenced primitive schemas.
type CanonicalTable = (Vec<Vec<u8>>, u32, BTreeSet<Primitive>);

/// Export a canonical dependency table; no local allocation order participates in selection.
fn table(cas: &Cas, root: Entity, budget: &mut Budget) -> Result<CanonicalTable, Fault> {
    let root_key = key(cas, &root, budget)?;
    let mut records = gather(cas, root, budget)?;
    let mut assigned = BTreeMap::new();
    let mut unique = BTreeMap::new();
    let mut bytes = Vec::new();
    let rules: BTreeSet<_> = records.values().filter_map(|record| record.rule).collect();
    let mut manifest: Vec<_> = rules.iter().copied().collect();
    manifest.sort_by_key(|rule| rule.specification().key);
    for record in records.values_mut() {
        if let Some(rule) = record.rule {
            let index = manifest
                .iter()
                .position(|value| *value == rule)
                .ok_or_else(invalid)?;
            let index = u32::try_from(index).map_err(|_| invalid())?;
            record
                .payload
                .get_mut(..4)
                .ok_or_else(invalid)?
                .copy_from_slice(&index.to_le_bytes());
        }
    }
    while !records.is_empty() {
        let mut least: Option<(Key, Vec<u8>)> = None;
        for (id, record) in &records {
            budget.work(1, AT)?;
            if let Some(candidate) = ready(cas, record, &assigned, budget)?
                && least
                    .as_ref()
                    .is_none_or(|(_, previous)| candidate < *previous)
            {
                least = Some((id.clone(), candidate));
            }
        }
        let (id, descriptor) = least.ok_or_else(invalid)?;
        records.remove(&id).ok_or_else(invalid)?;
        let index = if let Some(index) = unique.get(&descriptor) {
            *index
        } else {
            let index = u32::try_from(bytes.len()).map_err(|_| invalid())?;
            bytes.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            bytes.push(descriptor.clone());
            unique.insert(descriptor.clone(), index);
            index
        };
        assigned.insert(id, (index, descriptor));
    }
    let root = assigned.get(&root_key).ok_or_else(invalid)?.0;
    Ok((bytes, root, rules))
}

/// Encode a theorem DAG with a sorted manifest; primitive indices are patched by the record
/// builder.
pub(crate) fn encode(cas: &Cas, proof: LogicalId, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let application = crate::proof_view::application(cas, proof, budget)?;
    let root = Entity::Conclusion {
        proof: Some(proof),
        judgment: application.judgment,
        hypotheses: application.conditions.hypotheses,
    };
    let (records, root, rules) = table(cas, root, budget)?;
    envelope(records, root, &rules, b"NRZCAS02", budget)
}

/// A portable theorem comparison includes precisely its complete context and residual tuple set.
pub(crate) fn judgment_key(
    cas: &Cas,
    judgment: &Judgment,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let root = Entity::Conclusion {
        proof: None,
        judgment: judgment.clone(),
        hypotheses: Vec::new(),
    };
    let (records, root, rules) = table(cas, root, budget)?;
    envelope(records, root, &rules, b"NRZJDG02", budget)
}

/// Build a complete portable syntax key without exposing allocation-dependent logical identifiers.
pub(crate) fn syntax_key(cas: &Cas, id: LogicalId, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let (records, root, rules) = table(cas, Entity::Object(id), budget)?;
    envelope(records, root, &rules, b"NRZSYN02", budget)
}

/// Frame the already canonical records with their exact primitive manifest.
fn envelope(
    records: Vec<Vec<u8>>,
    root: u32,
    rules: &BTreeSet<Primitive>,
    magic: &[u8],
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut manifest: Vec<_> = rules.iter().map(|rule| rule.specification()).collect();
    manifest.sort_by_key(|rule| rule.key);
    let mut out = Encoder::witness(budget, AT);
    out.raw(magic)?;
    out.count(manifest.len())?;
    for rule in &manifest {
        out.name(rule.key)?;
        crate::wire::index(rule.version, &mut out)?;
    }
    out.count(records.len())?;
    for record in records {
        out.raw(&record)?;
    }
    crate::wire::index(root, &mut out)?;
    Ok(out.finish())
}

/// Keep exact cell rows aligned with the portable structural order of cover members.
fn reorder_rows(
    cas: &Cas,
    record: &Record,
    assigned: &BTreeMap<Key, (u32, Vec<u8>)>,
    payload: &mut [u8],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (start, width, members) in &record.row_sets {
        let length = width.checked_mul(members.len()).ok_or_else(invalid)?;
        let end = start.checked_add(length).ok_or_else(invalid)?;
        budget.check(BudgetKind::StagedPayloadBytes, size(length, AT)?, AT)?;
        budget.work(
            size(length, AT)?
                .checked_mul(size(members.len(), AT)?)
                .ok_or_else(invalid)?,
            AT,
        )?;
        let source = payload.get(*start..end).ok_or_else(invalid)?.to_vec();
        let mut order = Vec::new();
        order
            .try_reserve_exact(members.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (position, member) in members.iter().enumerate() {
            let descriptor = &assigned
                .get(&key(cas, member, budget)?)
                .ok_or_else(invalid)?
                .1;
            order.push((descriptor, position));
        }
        order.sort_by(|left, right| left.0.cmp(right.0));
        for (target, (_, original)) in order.into_iter().enumerate() {
            let from = original.checked_mul(*width).ok_or_else(invalid)?;
            let source_end = from.checked_add(*width).ok_or_else(invalid)?;
            let target = start
                .checked_add(target.checked_mul(*width).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            let target_end = target.checked_add(*width).ok_or_else(invalid)?;
            payload
                .get_mut(target..target_end)
                .ok_or_else(invalid)?
                .copy_from_slice(source.get(from..source_end).ok_or_else(invalid)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Entity, Key, Record, ready};
    use crate::budget::{Budget, Limits};
    use crate::domain_model::Layer;
    use crate::graph::{Cas, LogicalId, VertexKind};
    use std::collections::BTreeMap;

    /// Ready descriptors use child structural bytes rather than local serial order.
    /// ´test:unit:ready-reference-order´
    #[test]
    fn ready_reference_order() -> Result<(), Box<dyn std::error::Error>> {
        let cas = Cas::new(Limits::default())?;
        let mut budget = Budget::new(Limits::default())?;
        let left = LogicalId {
            kind: VertexKind::Region,
            serial: 9,
        };
        let right = LogicalId {
            kind: VertexKind::Region,
            serial: 2,
        };
        let record = Record {
            tag: 1,
            layer: Layer::Logic,
            space: None,
            payload: vec![0; 8],
            references: vec![(0, Entity::Object(left)), (4, Entity::Object(right))],
            sets: vec![vec![0, 1]],
            row_sets: Vec::new(),
            rule: None,
        };
        let mut assigned = BTreeMap::new();
        assigned.insert(Key::Object(left), (7, vec![2]));
        assert!(ready(&cas, &record, &assigned, &mut budget)?.is_none());
        assigned.insert(Key::Object(right), (4, vec![1]));
        let descriptor =
            ready(&cas, &record, &assigned, &mut budget)?.ok_or("ready record withheld")?;
        assert_eq!(
            descriptor.get(11..),
            Some([4, 0, 0, 0, 7, 0, 0, 0].as_slice())
        );
        Ok(())
    }
}
