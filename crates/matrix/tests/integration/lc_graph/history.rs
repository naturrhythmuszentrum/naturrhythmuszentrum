// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-supplied byte limits include surviving public delivery history.
use super::fixture::{Result, Setup};
use nrz_bindings::{
    Limits, Registry,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_matrix::*;
use nrz_scalar::Scalar;

/// One extra witness word adds eight bytes to the independently enumerated admission fixture.
const CAPACITY: LcGraphCapacities = LcGraphCapacities {
    nodes: 64,
    edges: 0,
    scopes: 0,
    drive: 100,
    records: 3,
    words: 19,
    witnesses: 1,
};

/// Only the bounded sink survives graph abandonment; admission still sees it on the original account.
pub(super) fn run<P: Scalar + 'static>() -> Result {
    let mut baseline = Setup::<P>::new(false)?;
    let Residence::Empty {
        retained_bytes: native,
    } = baseline.heap.residence(&mut baseline.account, 100_000)?
    else {
        return Err("empty original heap".into());
    };
    drop(baseline);
    // O shared376 + nodes8704 + ready8 + stack1024 + marks512 =10624.
    // K shared144 + records264 + words152 + registration24 =584.
    // Adapter144, gate Rc16+72 and sixty-four native slots with probe words.
    let covered = 10_624 + 584 + 144 + 16 + 72 + 64 * (size_of::<LcGraphNode<'_, P, 5>>() + 8);
    let limit = u64::try_from(native + covered + 8)?;
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = Setup::<P>::with_limits(
        false,
        false,
        Limits {
            payload_bytes: limit,
            ..Limits::default()
        },
        [2, 3, 2],
    )?;
    let first = admit(&inventory, &axes, &registry, &heap, &mut account)?;
    let observed = LcGraphSnapshot::inspect(&[first.borrowed()], &heap, &mut account)?;
    assert_eq!(
        observed.graph_bytes + observed.observer_bytes + observed.delivery_history_bytes,
        covered + 8
    );
    let witness = abandon(first, &mut heap, &mut account)?;
    assert_eq!(
        witness.status(&mut account)?,
        LcGraphDeliveryStatus::Abandoned
    );
    // Sink shared144 + records264 + words152 + witness8 = 568; registration is another24.
    assert_eq!(
        LcGraphSnapshot::inspect(&[], &heap, &mut account)?.delivery_history_bytes,
        592
    );
    let declaration = inventory
        .input(0)
        .ok_or("original source")?
        .source(&axes, &mut account)?;
    let refused = LcGraphOwner::admit(
        vec![LcGraphNode::input(declaration)],
        &[1],
        CAPACITY,
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    );
    let Err(LcGraphAdmissionFailure {
        nodes,
        error: LcGraphError::Native(LoanError::Budget(error)),
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
    assert_eq!(
        LcGraphSnapshot::inspect(&[], &heap, &mut account)?.delivery_history_bytes,
        24
    );
    let recovered = LcGraphOwner::admit(
        nodes,
        &[1],
        CAPACITY,
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    )
    .map_err(|LcGraphAdmissionFailure { error, .. }| error)?;
    drop(recovered);
    Ok(())
}

/// Fresh declarations borrow the same original inventory and unchanged semantic Registry.
fn admit<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphOwner<'i, P, 5>> {
    let declaration = inventory
        .input(0)
        .ok_or("original source")?
        .source(axes, account)?;
    Ok(LcGraphOwner::admit(
        vec![LcGraphNode::input(declaration)],
        &[1],
        CAPACITY,
        inventory,
        registry,
        &[],
        heap,
        account,
    )
    .map_err(|LcGraphAdmissionFailure { error, .. }| error)?)
}

/// The required terminal receipt remains unacknowledged when its graph owner disappears.
fn abandon<P: Scalar + 'static>(
    mut owner: LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphWitness> {
    let input = owner.handle(0, account)?;
    let LcGraphEvent::Terminal {
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
