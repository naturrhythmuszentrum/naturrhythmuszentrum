// SPDX-License-Identifier: AGPL-3.0-only
//! Sequential owner duration never grows the fixed pool or erases a surviving sink debit.

use super::super::{
    admission::Refusal,
    drive::Event,
    node::Node,
    owner::{Capacities, Owner},
};
use super::{boundaries, fixture::TestResult};
use crate::loan::graph::{sink, walk::Error};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use crate::{ExecutionAccount, Heap, LcInputInventory, LcShapeAxes, LoanError};
use nrz_bindings::{
    Limits, Registry,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_scalar::Scalar;

/// One extra witness word adds eight bytes to the independently enumerated admission fixture.
const CAPACITY: Capacities = Capacities {
    nodes: 64,
    edges: 0,
    scopes: 0,
    drive: 100,
    records: 3,
    words: 19,
    witnesses: 1,
};

/// Only the bounded sink survives graph abandonment; admission still sees it on the original account.
pub(super) fn run<P: Scalar + 'static>() -> TestResult {
    let baseline = setup::<P>(Limits::default(), [2, 3, 2])?;
    let native = baseline.heap.resource_bytes()?;
    drop(baseline);
    let (covered, _) = boundaries::inventory::<P>();
    let limit = u64::try_from(native + covered + 8)?;
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = setup::<P>(
        Limits {
            payload_bytes: limit,
            ..Limits::default()
        },
        [2, 3, 2],
    )?;
    let first = admit(&inventory, &axes, &registry, &heap, &mut account)?;
    assert_eq!(first.graph_bytes(&[], &mut account)?, covered + 8);
    let witness = abandon(first, &mut heap, &mut account)?;
    assert_eq!(witness.status(&mut account)?, sink::Status::Abandoned);
    // Sink shared144 + records264 + words152 + witness8 = 568; registration is another24.
    assert_eq!(sink::live_bytes(&account)?, 592);
    let declaration = inventory
        .input(0)
        .ok_or("original source")?
        .source(&axes, &mut account)?;
    let refused = Owner::admit(
        vec![Node::input(declaration)],
        &[1],
        CAPACITY,
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    );
    let Err(Refusal {
        nodes,
        error: Error::Native(LoanError::Budget(error)),
    }) = refused
    else {
        return Err("surviving history admission boundary".into());
    };
    let FaultKind::Exhausted {
        budget,
        needed,
        limit: actual,
        ..
    } = error.kind
    else {
        return Err("retained history dimension".into());
    };
    assert_eq!(budget, BudgetKind::PayloadBytes);
    assert_eq!((needed, actual), (limit + 568, limit));
    let before = account.usage();
    drop(witness);
    assert_eq!(account.usage(), before);
    assert_eq!(sink::live_bytes(&account)?, 24);
    let recovered = Owner::admit(
        nodes,
        &[1],
        CAPACITY,
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    drop(recovered);
    for _ in 0..16 {
        let owner = admit(&inventory, &axes, &registry, &heap, &mut account)?;
        assert_eq!(owner.graph_bytes(&[], &mut account)?, covered + 8);
        assert_eq!(owner.backing.nodes.capacity(), 64);
        let report = owner.originals(&mut account)?;
        let original = crate::loan::lc_shape::exact_scope::ExactScope::new(&inventory, &registry);
        originals(&report, &original)?;
        drop(owner);
        assert!(report.registry().same_owner(&original.registry()));
        assert_eq!(sink::live_bytes(&account)?, 24);
    }
    assert_eq!(heap.resource_bytes()?, native);
    Ok(())
}

/// Fresh declarations borrow the same original inventory and unchanged semantic Registry.
fn admit<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult<Owner<'i, P, 5>> {
    let declaration = inventory
        .input(0)
        .ok_or("original source")?
        .source(axes, account)?;
    Ok(Owner::admit(
        vec![Node::input(declaration)],
        &[1],
        CAPACITY,
        inventory,
        registry,
        &[],
        heap,
        account,
    )
    .map_err(|Refusal { error, .. }| error)?)
}

/// The required terminal receipt remains unacknowledged when its graph owner disappears.
fn abandon<P: Scalar + 'static>(
    mut owner: Owner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<sink::witness::Witness> {
    let input = owner.handle(0, account)?;
    let Event::Terminal {
        evidence, control, ..
    } = owner.cancel_node(input, heap, account)?
    else {
        return Err("cancelled original expectation".into());
    };
    control?;
    let witness = owner.witness(evidence, account)?;
    let before = account.usage();
    drop(owner);
    assert_eq!(account.usage(), before);
    Ok(witness)
}

/// Shared originals keep their identities and logical allowances separate from unknown capacity.
fn originals<P: Scalar>(
    report: &crate::loan::lc_shape::exact_scope::ExactScope<'_, P, 5>,
    original: &crate::loan::lc_shape::exact_scope::ExactScope<'_, P, 5>,
) -> TestResult {
    assert!(report.inventory().same_owner(&original.inventory()));
    assert!(report.registry().same_owner(&original.registry()));
    let first = report.input(0).ok_or("reported original position")?;
    for position in 0..5 {
        let use_row = report.input(position).ok_or("reported original use")?;
        assert_eq!(use_row.position, position);
        assert!(use_row.question.same_owner(&first.question));
        assert!(use_row.relation.same_owner(&first.relation));
        assert_eq!(use_row.question.measure.backing_capacity_bytes, None);
        assert!(
            use_row
                .question
                .measure
                .logical_allowance_bytes
                .is_some_and(|bytes| bytes != 0)
        );
    }
    Ok(())
}
