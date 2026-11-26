// SPDX-License-Identifier: AGPL-3.0-only
//! The first consumer receives the original pin only after the additional use settles.

use super::super::{
    admission::Refusal,
    drive::Event,
    native::Failure,
    node::{Node, State},
    owner::{Capacities, Owner},
};
use super::fixture::{self, TestResult};
use crate::LoanError;
use crate::loan::lc_shape::tests::output::{Setup, setup};
use nrz_bindings::Limits;

/// One failed retention consumes exactly one additional consumer attempt, never the producer's.
pub(super) fn retry(attempts: u64) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        ..
    } = setup::<f64>(Limits::default(), [2, 3, 2])?;
    let schema = inventory.input(0).ok_or("original input")?;
    let nodes = vec![
        Node::input(schema.source(&axes, &mut account)?),
        Node::mapped(
            0,
            schema,
            &axes,
            fixture::mapping(schema, &axes, &mut heap, &mut account)?,
        ),
        Node::mapped(
            0,
            schema,
            &axes,
            fixture::mapping(schema, &axes, &mut heap, &mut account)?,
        ),
    ];
    let mut owner = Owner::admit(
        nodes,
        &[8, 8, attempts],
        Capacities {
            nodes: 3,
            edges: 2,
            scopes: 0,
            drive: 100,
            records: 6 + 16 + usize::try_from(attempts)?,
            words: 33 + 8 * (16 + usize::try_from(attempts)?),
            witnesses: 0,
        },
        &inventory,
        &registry,
        &[],
        &heap,
        &mut account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    let input = fixture::publish_input(&mut owner, 0, &mut heap, &mut account)?;
    let target = owner.handle(2, &mut account)?;
    let injection = crate::loan::fault::Injection::after(0);
    let event = owner.drive_one(&mut heap, &mut account, &[])?;
    drop(injection);
    let Event::Refused {
        node,
        error: Failure::Native(LoanError::AllocationRefused),
        probe: Some(probe),
        control,
    } = event
    else {
        return Err(format!("additional retention: {event:?}").into());
    };
    control?;
    assert_eq!(node, target);
    owner.acknowledge_probe(probe, &mut account)?;
    assert!(owner.backing.nodes[0].pin.is_some());
    assert!(owner.backing.nodes[1].pin.is_none());
    assert!(owner.backing.nodes[2].pin.is_none());
    assert_eq!(owner.backing.core.reservation.backing.nodes[0].attempts, 7);
    assert_eq!(owner.backing.core.reservation.backing.nodes[1].attempts, 8);
    assert_eq!(
        owner.backing.core.reservation.backing.nodes[2].attempts,
        attempts - 1
    );
    owner.begin_capacity_epoch(&mut account)?;
    let event = owner.drive_wait_one(&mut heap, &mut account, &[])?;
    settle(&mut owner, event, target, attempts, &heap, &mut account)?;
    assert!(owner.backing.nodes[1].pin.is_none());
    assert_eq!(owner.backing.nodes[2].pin.is_some(), attempts == 2);
    assert_eq!(owner.backing.nodes[2].waiting_for, None);
    let Event::Terminal {
        node,
        evidence,
        control,
    } = owner.drive_one(&mut heap, &mut account, &[])?
    else {
        return Err("resumed producer publication".into());
    };
    assert_eq!(node, input);
    control?;
    assert!(owner.backing.nodes[0].pin.is_none());
    assert!(owner.backing.nodes[1].pin.is_some());
    owner.acknowledge_outcome(node, evidence, &heap, &mut account)?;
    Ok(())
}

/// Exhaustion consumes no replacement attempt and returns both fixed receipt obligations.
fn settle<'i>(
    owner: &mut Owner<'i, f64, 5>,
    event: Event<'i, f64, 5>,
    target: crate::loan::graph::walk::Handle,
    attempts: u64,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    match event {
        Event::Advanced {
            node,
            probe,
            control,
        } => {
            assert_eq!(attempts, 2);
            assert_eq!(node, target);
            control?;
            owner.acknowledge_probe(probe, account)?;
        }
        Event::AttemptsExhausted {
            node,
            evidence,
            outcome,
            control,
        } => {
            assert_eq!(attempts, 1);
            assert_eq!(node, target);
            control?;
            assert_eq!(
                owner.backing.nodes[target.slot].state,
                State::AttemptsExhausted
            );
            owner.acknowledge_probe(evidence, account)?;
            // The saved terminal outcome is acknowledged after its notification gate commits.
            assert_eq!(owner.pull(outcome, account)?.bytes(), 41);
            assert!(
                owner
                    .acknowledge_outcome(node, outcome, heap, account)
                    .is_err()
            );
        }
        other => return Err(format!("retention retry: {other:?}").into()),
    }
    assert_eq!(
        owner.backing.core.reservation.backing.nodes[target.slot].attempts,
        0
    );
    Ok(())
}
