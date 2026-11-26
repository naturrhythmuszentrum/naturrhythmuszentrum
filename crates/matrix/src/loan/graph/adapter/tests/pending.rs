// SPDX-License-Identifier: AGPL-3.0-only
//! A checked preparation survives native reservation refusal and retries on the original account.

use super::super::{
    drive::Event,
    native::{Failure, Native},
    owner::Owner,
};
use super::fixture::{self, TestResult};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Allocation refusal returns the same law allocation; the next bounded epoch acquires pending storage.
pub(super) fn run<P: Scalar + 'static>(allocation: usize) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(nrz_bindings::Limits::default(), [2, 3, 2])?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    for position in 0..5 {
        fixture::publish_input(&mut owner, position, &mut heap, &mut account)?;
    }
    prepared(&mut owner, &mut heap, &mut account)?;
    let omega = owner.handle(10, &mut account)?;
    let Some(Native::OmegaPrepared(prepared)) = &owner.backing.nodes[10].native else {
        return Err("checked preparation".into());
    };
    let original = Rc::as_ptr(&prepared.correspondence);
    let before = owner.backing.core.reservation.backing.nodes[10].attempts;
    let injection = crate::loan::fault::Injection::after(allocation);
    let event = next_native(&mut owner, &mut heap, &mut account)?;
    drop(injection);
    let Event::Refused {
        node,
        error: Failure::Native(error),
        probe: Some(probe),
        control,
    } = event
    else {
        return Err(format!("pending allocation: {event:?}").into());
    };
    if allocation == 0 {
        assert!(matches!(error, LoanError::AllocationRefused));
    } else {
        assert!(matches!(error, LoanError::Heap(crate::HeapError::Capacity)));
    }
    assert_eq!(
        owner.backing.gate.loan(&error),
        super::super::refusal::Class::AllocationWait
    );
    assert_eq!(node, omega);
    control?;
    owner.acknowledge_probe(probe, &mut account)?;
    let Some(Native::OmegaPrepared(prepared)) = &owner.backing.nodes[10].native else {
        return Err("preserved prepared owner".into());
    };
    assert_eq!(Rc::as_ptr(&prepared.correspondence), original);
    assert_eq!(
        owner.backing.core.reservation.backing.nodes[10].attempts,
        before - 1
    );
    owner.begin_capacity_epoch(&mut account)?;
    owner.cancel_capacity_scan(&mut account)?;
    assert_eq!(
        owner.backing.core.reservation.backing.nodes[10].attempts,
        before - 1
    );
    owner.begin_capacity_epoch(&mut account)?;
    let Event::Advanced {
        node,
        probe,
        control,
    } = owner.drive_wait_one(&mut heap, &mut account, &[])?
    else {
        return Err("pending native retry".into());
    };
    assert_eq!(node, omega);
    control?;
    owner.acknowledge_probe(probe, &mut account)?;
    assert_eq!(
        owner.backing.core.reservation.backing.nodes[10].attempts,
        before - 2
    );
    let Some(Native::OmegaPending(pending)) = &owner.backing.nodes[10].native else {
        return Err("native pending continuation".into());
    };
    let ticket = pending.ticket();
    super::failures::cancelled(&mut owner, &mut heap, &mut account)?;
    assert!(matches!(
        heap.take_producer_outcome(ticket, &mut account)?,
        crate::ProducerOutcome::AlreadyDelivered
    ));
    assert!(matches!(
        owner.backing.nodes[10].native,
        Some(Native::Cancelled)
    ));
    Ok(())
}

/// Readiness enters construction once and never fabricates an output to bypass the checked join.
pub(super) fn prepared<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    for _ in 0..128 {
        if matches!(
            owner.backing.nodes[10].native,
            Some(Native::OmegaPrepared(_))
        ) {
            return Ok(());
        }
        match owner.drive_one(heap, account, &[])? {
            Event::Advanced { probe, control, .. } => {
                control?;
                owner.acknowledge_probe(probe, account)?;
            }
            Event::Terminal {
                node,
                evidence,
                control,
            } => {
                assert!(node.slot < 10);
                control?;
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            Event::Control => {}
            other => return Err(format!("checked preparation progression: {other:?}").into()),
        }
    }
    Err("bounded preparation did not finish".into())
}

/// Existing control cleanup may precede the next eligible native reservation.
pub(super) fn next_native<'i, P: Scalar + 'static>(
    owner: &mut Owner<'i, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<Event<'i, P, 5>> {
    for _ in 0..32 {
        let event = owner.drive_one(heap, account, &[])?;
        if !matches!(event, Event::Control) {
            return Ok(event);
        }
    }
    Err("bounded native selection".into())
}
