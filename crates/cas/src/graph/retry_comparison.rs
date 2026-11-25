// SPDX-License-Identifier: AGPL-3.0-only
//! Full retained-state normalization for the journal retry fixture.

use std::collections::{BTreeMap, BTreeSet};

use super::{Cas, Handle, LogicalId, Vertex, VertexKind, vertex_key};
use crate::testing::common::{TestResult, default_budget};

/// Preserve prefix identities and pair every suffix record through returned handles.
fn identities(
    prefix: &[LogicalId],
    actual: &Cas,
    actual_handles: &[Handle],
    expected: &Cas,
    expected_handles: &[Handle],
) -> TestResult<BTreeMap<LogicalId, LogicalId>> {
    let mut ids: BTreeMap<_, _> = prefix.iter().map(|id| (*id, *id)).collect();
    assert_eq!(actual_handles.len(), expected_handles.len());
    let mut budget = default_budget()?;
    for (left, right) in actual_handles.iter().zip(expected_handles) {
        let left = actual.checked(left, &mut budget)?;
        let right = expected.checked(right, &mut budget)?;
        pair(&mut ids, left, right);
        // A returned proof owns an application that has no separate public return value.
        if let (Vertex::Proof(left), Vertex::Proof(right)) = (
            actual.state.node(left, &mut budget)?,
            expected.state.node(right, &mut budget)?,
        ) {
            pair(&mut ids, *left, *right);
        }
    }
    assert_eq!(
        ids.keys().copied().collect::<BTreeSet<_>>(),
        actual.state.indices.keys().copied().collect()
    );
    let targets: BTreeSet<_> = ids.values().copied().collect();
    assert_eq!(targets.len(), ids.len());
    assert_eq!(targets, expected.state.indices.keys().copied().collect());
    Ok(ids)
}

/// A repeated handle or prefix hit must keep its established correspondence.
fn pair(ids: &mut BTreeMap<LogicalId, LogicalId>, left: LogicalId, right: LogicalId) {
    assert_eq!(left.kind, right.kind);
    if let Some(previous) = ids.insert(left, right) {
        assert_eq!(previous, right);
    }
}

/// Missing identities are fixture/comparison failures, never implicit identity mappings.
fn mapped(ids: &BTreeMap<LogicalId, LogicalId>, id: LogicalId) -> TestResult<LogicalId> {
    ids.get(&id)
        .copied()
        .ok_or_else(|| "unmapped retry identity".into())
}

/// Normalize a copy without reconstructing graph edges, child order or derived maps.
///
/// Only occurrences and proof roots reference suffix records in this fixture. Other
/// vertex weights stay untouched and must compare exactly, including cached fields.
/// Validate every original interning entry before translating its key; extra or stale
/// entries must fail instead of being silently repaired by rebuilding the index.
pub(crate) fn normalized(
    prefix: &[LogicalId],
    actual: &Cas,
    actual_handles: &[Handle],
    expected: &Cas,
    expected_handles: &[Handle],
) -> TestResult<Cas> {
    let ids = identities(prefix, actual, actual_handles, expected, expected_handles)?;
    let mut budget = default_budget()?;
    let mut copy = actual.full_copy_for_test();
    for vertex in copy.state.graph.node_weights_mut() {
        match vertex {
            Vertex::Occurrence(value) => {
                value.expression = mapped(&ids, value.expression)?;
                value.source = value.source.map(|id| mapped(&ids, id)).transpose()?;
                value.provenance = value.provenance.map(|id| mapped(&ids, id)).transpose()?;
                for child in &mut value.children {
                    *child = mapped(&ids, *child)?;
                }
            }
            Vertex::Proof(application) => *application = mapped(&ids, *application)?,
            _ => {}
        }
    }
    copy.state.indices = actual
        .state
        .indices
        .iter()
        .map(|(id, index)| Ok((mapped(&ids, *id)?, *index)))
        .collect::<TestResult<_>>()?;
    copy.state.sources = actual
        .state
        .sources
        .iter()
        .map(|(key, id)| Ok((key.clone(), mapped(&ids, *id)?)))
        .collect::<TestResult<_>>()?;
    copy.state.parents = actual
        .state
        .parents
        .iter()
        .map(|(child, parent)| Ok((mapped(&ids, *child)?, mapped(&ids, *parent)?)))
        .collect::<TestResult<_>>()?;
    copy.state.policies = actual
        .state
        .policies
        .iter()
        .map(|(key, id)| Ok((key.clone(), mapped(&ids, *id)?)))
        .collect::<TestResult<_>>()?;
    let mut remaining = std::mem::take(&mut copy.state.intern);
    for id in actual.state.indices.keys() {
        if matches!(id.kind, VertexKind::Occurrence | VertexKind::Binder) {
            continue;
        }
        let (_, old_key) = vertex_key(
            actual.state.node(*id, &mut budget)?,
            &actual.state,
            &mut default_budget()?,
        )?;
        assert_eq!(
            remaining.get(
                &old_key,
                0,
                &mut default_budget()?,
                crate::error::Location::new("retry comparison"),
            )?,
            Some(id)
        );
        remaining.remove_prepaid(old_key.into());
        let target = mapped(&ids, *id)?;
        let (_, key) = vertex_key(
            copy.state.node(target, &mut budget)?,
            &copy.state,
            &mut default_budget()?,
        )?;
        let stored = key.into();
        let at = crate::error::Location::new("retry comparison");
        let (position, _) =
            copy.state
                .intern
                .prepare_insert_key(&stored, false, &mut default_budget()?, at)?;
        copy.state
            .intern
            .insert_prepared_key(position, stored, target, &copy.token);
    }
    assert_eq!(remaining.len(), 0);
    Ok(copy)
}
