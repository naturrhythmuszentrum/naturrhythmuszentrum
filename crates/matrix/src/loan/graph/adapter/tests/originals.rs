// SPDX-License-Identifier: AGPL-3.0-only
//! Shared versus independently checked equal originals retain distinct report identities.
use super::{
    super::{
        admission::Refusal,
        node::Node,
        owner::{Capacities, Owner},
        snapshot::Snapshot,
    },
    fixture::TestResult,
};
use crate::{ExecutionAccount, Heap, LcInputInventory, LcShapeAxes};
use nrz_bindings::Registry;
use nrz_scalar::Scalar;

/// Common live graph snapshots do not absorb original logical allowances into physical capacity.
pub(super) fn run<'i, P: Scalar + 'static>(
    old: &Owner<'i, P, 5>,
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let original = inventory.input(0).ok_or("original input")?.question();
    let independent =
        crate::loan::lc_shape::tests::fixture::inventory_question(true, true, &mut account.ledger)?;
    let shared = LcInputInventory::<P, 5>::declare(original, [2, 3, 2], account)?;
    let distinct = LcInputInventory::<P, 5>::declare(&independent, [2, 3, 2], account)?;
    let same_owner = admit(&shared, axes, registry, heap, account, &[old.borrowed()])?;
    let separate_owner = admit(
        &distinct,
        axes,
        registry,
        heap,
        account,
        &[old.borrowed(), same_owner.borrowed()],
    )?;
    let original_report = old.originals(account)?;
    let same = same_owner.originals(account)?;
    let separate = separate_owner.originals(account)?;
    let first = original_report.input(0).ok_or("reported original")?;
    let repeated = same.input(0).ok_or("reported shared")?;
    let other = separate.input(0).ok_or("reported independent")?;
    assert!(first.question.same_owner(&repeated.question));
    assert!(first.relation.same_owner(&repeated.relation));
    assert!(!first.question.same_owner(&other.question));
    assert!(!first.relation.same_owner(&other.relation));
    assert_eq!(
        first.question.measure.logical_allowance_bytes,
        other.question.measure.logical_allowance_bytes
    );
    assert_eq!(first.question.measure.backing_capacity_bytes, None);
    assert!(original_report.registry().same_owner(&same.registry()));
    assert!(original_report.registry().same_owner(&separate.registry()));
    let snapshot = Snapshot::inspect(
        &[
            old.borrowed(),
            same_owner.borrowed(),
            separate_owner.borrowed(),
            old.borrowed(),
        ],
        heap,
        account,
    )?;
    assert_eq!(snapshot.graph_owners, 3);
    assert_eq!(snapshot.observer_bytes, 3 * (72 + 16));
    assert_eq!(snapshot.original_exact_capacity_bytes, None);
    let before = first.question.measure;
    drop(same_owner);
    drop(separate_owner);
    assert_eq!(
        original_report
            .input(0)
            .ok_or("retained original")?
            .question
            .measure,
        before
    );
    assert!(same.registry().same_owner(&separate.registry()));
    Ok(())
}

/// The distinct inventories are explicit caller borrows; this graph owns no hidden exact copy.
fn admit<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &Heap,
    account: &mut ExecutionAccount,
    prior: &[super::super::owner::Borrowed<'_>],
) -> TestResult<Owner<'i, P, 5>> {
    let node = Node::input(inventory.input(0).ok_or("source")?.source(axes, account)?);
    Ok(Owner::admit(
        vec![node],
        &[1],
        Capacities {
            nodes: 1,
            edges: 0,
            scopes: 0,
            drive: 10,
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
    .map_err(|Refusal { error, .. }| error)?)
}
