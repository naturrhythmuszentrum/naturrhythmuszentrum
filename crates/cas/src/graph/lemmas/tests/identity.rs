// SPDX-License-Identifier: AGPL-3.0-only
//! Direct anonymous publication shares the token allocator and restores retained accounts.

use super::super::{AnonymousLemma, PendingJudgment};
use super::fixture::{Fixture, ledger, retain_graph};
use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex, VertexKind};
use crate::testing::common::TestResult;

/// Refusal after all independent checkers and direct append paths have succeeded.
const AT: Location = Location::new("anonymous identity control");

/// Capture actual graph handles without depending on physical indices or payload order.
fn anonymous(cas: &Cas) -> Vec<Handle> {
    cas.state
        .indices
        .keys()
        .filter(|id| id.kind == VertexKind::AnonymousLemma)
        .map(|id| cas.handle(*id))
        .collect()
}

/// Successful top-level checks publish every inventory before an enclosing refusal is tested.
fn checked_copy(f: &Fixture) -> TestResult<Cas> {
    let mut cas = f.cas.full_copy_for_test();
    retain_graph(
        &mut cas,
        &f.context,
        &f.one,
        &f.pi,
        &f.scalar,
        &mut ledger()?,
    )?;
    assert_eq!(cas.retained_judgment_count(), 4);
    Ok(cas)
}

/// The three checked inventories refer only to the original shared prefix.
fn staged(cas: &Cas) -> Vec<PendingJudgment> {
    cas.state
        .graph
        .node_weights()
        .filter_map(|node| {
            let Vertex::AnonymousLemma(node) = node else {
                return None;
            };
            match node.as_ref() {
                AnonymousLemma::Scalar(node) => {
                    Some(PendingJudgment::NewScalar(node.inventory.clone()))
                }
                AnonymousLemma::Pi(node) => Some(PendingJudgment::NewPi(node.inventory.clone())),
                AnonymousLemma::Enclosure(node) => {
                    Some(PendingJudgment::NewEnclosure(node.inventory.clone()))
                }
                _ => None,
            }
        })
        .collect()
}

/// Constant, scalar, Machin and enclosure append paths discard identities but restore occupancy.
/// ´test:unit:logical-identity-direct-judgments´
#[test]
fn logical_identity_direct_judgments() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = Fixture::new()?;
    let prepared = checked_copy(&f)?;
    let prefix = f.witness.proof().clone();
    let counts = f.cas.counts();
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let proofs = f.cas.state.proof_count(&mut budget)?;
    let mut stale = Vec::new();
    let fault = Fault::invalid(InvalidCode::Rule, AT);
    for _ in 0..3 {
        let mut budget = ledger()?;
        let mut spent = 0;
        let result: Result<(), Fault> = f.cas.transaction(&mut budget, |cas, budget| {
            cas.prepare_constant(&f.context, &f.one, budget)?;
            let mut pending = staged(&prepared);
            assert_eq!(pending.len(), 3);
            cas.publish_staged_judgments(&mut pending, budget)?;
            let suffix = anonymous(cas);
            assert_eq!(suffix.len(), 4);
            stale.extend(suffix);
            spent = budget.usage().work;
            Err(fault.clone())
        });
        assert_eq!(result.as_ref(), Err(&fault));
        assert_eq!(budget.usage().work, spent);
        assert!(spent > 256);
        assert_eq!(f.cas.counts(), counts);
        assert_eq!(f.cas.retained_judgment_count(), 0);
        assert_eq!(f.cas.retained_judgment_bytes(), 0);
        assert_eq!(f.cas.state.proof_count(&mut budget)?, proofs);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
        for handle in &stale {
            assert_eq!(
                f.cas
                    .checked(handle, &mut budget)
                    .err()
                    .ok_or("discarded judgment survived")?
                    .kind,
                FaultKind::Invalid(InvalidCode::Reference)
            );
        }
    }
    let vertices = u64::try_from(counts.vertices)? + 4;
    let mut budget = Budget::new(Limits {
        vertices,
        staged_vertices: vertices,
        proof_nodes: proofs,
        ..Limits::default()
    })?;
    f.retain_graph(&mut budget)?;
    assert_eq!(f.cas.retained_judgment_count(), 4);
    assert_eq!(f.cas.retained_judgment_bytes(), 4_600);
    assert_eq!(f.cas.counts().vertices, counts.vertices + 4);
    assert_eq!(f.cas.counts().payload_bytes, counts.payload_bytes + 4_600);
    let replacements = anonymous(&f.cas);
    for handle in &stale {
        for replacement in &replacements {
            assert_ne!(handle, replacement);
            assert!(replacement.logical_id().serial > handle.logical_id().serial);
        }
        assert_eq!(
            f.cas
                .checked(handle, &mut budget)
                .err()
                .ok_or("judgment reincarnated")?
                .kind,
            FaultKind::Invalid(InvalidCode::Reference)
        );
    }
    assert_eq!(f.witness.proof(), &prefix);
    f.cas.checked(&prefix, &mut budget)?;
    assert_eq!(
        f.cas.export_witness(&f.witness, &mut ledger()?)?,
        f.witness.bytes
    );
    let counts = f.cas.counts();
    f.retain_graph(&mut ledger()?)?;
    assert_eq!(anonymous(&f.cas), replacements);
    assert_eq!(f.cas.counts(), counts);
    Ok(())
}

/// A mixed suffix cannot publish partially when its complete serial reservation overflows.
/// ´test:unit:logical-identity-staged-boundary´
#[test]
fn logical_identity_staged_boundary() -> TestResult {
    let mut f = Fixture::new()?;
    let mut pending = staged(&checked_copy(&f)?);
    assert_eq!(pending.len(), 3);
    assert_eq!(
        f.cas
            .token
            .identities
            .reserve(VertexKind::AnonymousLemma, u64::MAX - 6, AT)?,
        4
    );
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    let result = f.cas.publish_staged_judgments(&mut pending, &mut ledger()?);
    assert_eq!(
        result,
        Err(Fault::capacity(
            BudgetKind::Vertices,
            Location::new("checked scalar records")
        ))
    );
    assert_eq!(pending.len(), 3);
    assert_eq!(f.cas.retained_judgment_count(), 0);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    drop(pending.pop());
    f.cas
        .publish_staged_judgments(&mut pending, &mut ledger()?)?;
    assert_eq!(pending.len(), 0);
    let serials = anonymous(&f.cas)
        .iter()
        .map(|handle| handle.logical_id().serial)
        .collect::<Vec<_>>();
    assert_eq!(serials, [u64::MAX - 2, u64::MAX - 1]);
    assert_eq!(f.cas.retained_judgment_count(), 2);
    let counts = f.cas.counts();
    assert_eq!(
        f.cas
            .prepare_constant(&f.context, &f.one, &mut ledger()?)
            .err()
            .ok_or("constant overflow accepted")?,
        Fault::capacity(BudgetKind::Vertices, super::super::AT)
    );
    assert_eq!(f.cas.counts(), counts);
    Ok(())
}

/// Complete constant keys preserve both high serials without changing their byte allowance.
/// ´test:unit:logical-identity-constant-keys´
#[test]
fn logical_identity_constant_keys() {
    let mut keys = std::collections::BTreeSet::new();
    for context in [0, u64::from(u32::MAX), u64::MAX - 1] {
        for original in [0, u64::from(u32::MAX), u64::MAX - 1] {
            let key = super::super::constant_key(
                LogicalId {
                    kind: VertexKind::Context,
                    serial: context,
                },
                LogicalId {
                    kind: VertexKind::Expression,
                    serial: original,
                },
            );
            assert_eq!(key.len(), 24);
            assert_eq!(&key[7..15], &context.to_le_bytes());
            assert_eq!(&key[16..24], &original.to_le_bytes());
            assert!(keys.insert(key));
        }
    }
}
