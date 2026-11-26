// SPDX-License-Identifier: AGPL-3.0-only
//! Equal originals have distinct ownership; borrowed Registry authority remains shared.
use super::fixture::Result;
use nrz_bindings::Registry;
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<'i, P: Scalar + 'static>(
    old: &LcGraphOwner<'i, P, 5>,
    shared: &'i LcInputInventory<P, 5>,
    distinct: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let first = admit(shared, axes, registry, &[old.borrowed()], heap, account)?;
    let second = admit(
        distinct,
        axes,
        registry,
        &[old.borrowed(), first.borrowed()],
        heap,
        account,
    )?;
    let original = old.originals(account)?;
    let shared = first.originals(account)?;
    let distinct = second.originals(account)?;
    let a = original.input(0).ok_or("original")?;
    let b = shared.input(0).ok_or("shared")?;
    let c = distinct.input(0).ok_or("distinct")?;
    assert!(a.question.same_owner(&b.question));
    assert!(!a.question.same_owner(&c.question));
    assert!(!a.relation.same_owner(&c.relation));
    assert_eq!(a.question.measure, c.question.measure);
    assert_eq!(a.relation.measure, c.relation.measure);
    assert!(original.registry().same_owner(&distinct.registry()));
    let report = LcGraphSnapshot::inspect(
        &[
            old.borrowed(),
            first.borrowed(),
            second.borrowed(),
            old.borrowed(),
        ],
        heap,
        account,
    )?;
    assert_eq!(report.graph_owners, 3);
    assert_eq!(report.observer_bytes, 3 * 88);
    assert_eq!(report.original_exact_capacity_bytes, None);
    assert_eq!(report.provenance_capacity_bytes, None);
    drop(first);
    drop(second);
    assert!(original.registry().same_owner(&distinct.registry()));
    Ok(())
}
fn admit<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    prior: &[LcGraphBorrowed<'_>],
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphOwner<'i, P, 5>> {
    let node = LcGraphNode::input(inventory.input(0).ok_or("input")?.source(axes, account)?);
    Ok(LcGraphOwner::admit(
        vec![node],
        &[1],
        LcGraphCapacities {
            nodes: 1,
            edges: 0,
            scopes: 0,
            drive: 100,
            records: 3,
            words: 19,
            witnesses: 0,
        },
        inventory,
        registry,
        prior,
        heap,
        account,
    )
    .map_err(|failure| failure.error)?)
}
