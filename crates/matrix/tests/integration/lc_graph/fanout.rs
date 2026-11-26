// SPDX-License-Identifier: AGPL-3.0-only
//! A caller-limited extra pin refuses, then retries after another public owner releases.
use super::fixture::{self, Result, Setup};
use nrz_bindings::Limits;
use nrz_matrix::*;

/// A refused retention can retry while preserving the original producer publication.
pub(super) fn run<P: nrz_scalar::Scalar + 'static>() -> Result {
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
            payload_bytes: super::capacity::LIMIT,
            ..Limits::default()
        },
        [2, 3, 2],
    )?;
    let mut owner = admit(&inventory, &axes, &registry, &mut heap, &mut account)?;
    let input = owner.handle(0, &mut account)?;
    let mut loan = fixture::request(&mut owner, input, &mut heap, &mut account)?;
    fixture::fill(&mut loan, 0, 2, &mut account)?;
    let LcGraphInputReturn::Published { control } =
        owner.return_input(input, loan, &mut heap, &mut account)
    else {
        return Err("fanout publication".into());
    };
    control?;
    let target = owner.handle(2, &mut account)?;
    // O1024; adapter144+observer88; K144+records2112+words1416+registration24.
    let graph = 1024 + 232 + 3 * (size_of::<LcGraphNode<'_, P, 5>>() + 8) + 3696;
    let ballast = super::capacity::fill(
        &owner,
        graph,
        &inventory,
        &registry,
        &mut heap,
        &mut account,
    )?;
    let event = owner.drive_one(&mut heap, &mut account, &[ballast.borrowed()])?;
    let LcGraphEvent::Refused {
        node,
        error:
            LcGraphNativeFailure::Native(LoanError::Heap(HeapError::ResourceBytes {
                staged: false,
                used,
                limit,
            })),
        probe: Some(probe),
        control,
    } = event
    else {
        return Err(format!("additional retention: {event:?}").into());
    };
    control?;
    assert_eq!(node, target);
    owner.acknowledge_probe(probe, &mut account)?;
    assert_eq!(limit, super::capacity::LIMIT);
    assert!(used > limit);
    assert_eq!(
        owner.state(target, &heap, &mut account)?,
        LcGraphState::ResourceWait
    );
    drop(ballast);
    owner.begin_capacity_epoch(&mut account)?;
    let event = owner.drive_wait_one(&mut heap, &mut account, &[])?;
    let LcGraphEvent::Advanced {
        node,
        probe,
        control,
    } = event
    else {
        return Err(format!("fanout retry: {event:?}").into());
    };
    assert_eq!(node, target);
    control?;
    owner.acknowledge_probe(probe, &mut account)?;
    let LcGraphEvent::Terminal {
        node,
        evidence,
        control,
    } = owner.drive_one(&mut heap, &mut account, &[])?
    else {
        return Err("resumed producer publication".into());
    };
    assert_eq!(node, input);
    control?;
    owner.acknowledge_outcome(node, evidence, &heap, &mut account)?;
    Ok(())
}

/// Three original nodes and two fixed incidences reserve both consumers' finite attempts.
fn admit<'i, P: nrz_scalar::Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i nrz_bindings::Registry,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphOwner<'i, P, 5>> {
    let attempts = 2;
    let schema = inventory.input(0).ok_or("original input")?;
    let nodes = vec![
        LcGraphNode::input(schema.source(axes, account)?),
        LcGraphNode::mapped(
            0,
            schema,
            axes,
            fixture::mapping(schema, axes, heap, account)?,
        ),
        LcGraphNode::mapped(
            0,
            schema,
            axes,
            fixture::mapping(schema, axes, heap, account)?,
        ),
    ];
    let owner = LcGraphOwner::admit(
        nodes,
        &[8, 8, attempts],
        LcGraphCapacities {
            nodes: 3,
            edges: 2,
            scopes: 0,
            drive: 100,
            records: 6 + 16 + usize::try_from(attempts)?,
            words: 33 + 8 * (16 + usize::try_from(attempts)?),
            witnesses: 0,
        },
        inventory,
        registry,
        &[],
        heap,
        account,
    )
    .map_err(|LcGraphAdmissionFailure { error, .. }| error)?;
    Ok(owner)
}
