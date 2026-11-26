// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit cancellation consumes each genuine checked native continuation without replaying it.
use super::{
    drive,
    fixture::{self, Result, Setup},
};
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    // Successful omega advances publish prepared, pending and evaluated continuations, in order.
    for advances in 1..=3 {
        let Setup {
            mut heap,
            mut account,
            inventory,
            axes,
            registry,
            radian,
            certificate,
            ..
        } = Setup::<P>::new(false)?;
        let (mut owner, masks) = fixture::admit(
            &inventory,
            &axes,
            &registry,
            &radian,
            &mut heap,
            &mut account,
            (&certificate, &(0..12).collect::<Vec<_>>(), false),
        )?;
        drive::issue(&mut owner, 2, &mut heap, &mut account)?;
        let omega = owner.handle(10, &mut account)?;
        advance(&mut owner, omega, advances, &mut heap, &mut account)?;
        let LcGraphEvent::Terminal {
            node,
            evidence,
            control,
        } = owner.cancel_node(omega, &mut heap, &mut account)?
        else {
            return Err("native continuation cancellation".into());
        };
        assert_eq!(node, omega);
        control?;
        assert_eq!(
            owner.state(omega, &heap, &mut account)?,
            LcGraphState::Cancelled
        );
        owner.acknowledge_outcome(node, evidence, &heap, &mut account)?;
        drain(&mut owner, &mut heap, &mut account)?;
        fixture::retire(masks, &mut heap, &mut account)?;
        drive::complete(&owner, &mut heap, &mut account)?;
    }
    Ok(())
}
fn advance<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    omega: LcGraphHandle,
    target: usize,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let mut advances = 0;
    for _ in 0..256 {
        match owner.drive_one(heap, account, &[])? {
            LcGraphEvent::Advanced {
                node,
                probe,
                control,
            } => {
                control?;
                owner.acknowledge_probe(probe, account)?;
                if node == omega {
                    advances += 1;
                    if advances == target {
                        return Ok(());
                    }
                }
            }
            LcGraphEvent::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            LcGraphEvent::Control => {}
            other => return Err(format!("native continuation: {other:?}").into()),
        }
    }
    Err("native continuation was not reached".into())
}
fn drain<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let output = owner.handle(11, account)?;
    let mut delivered = false;
    for _ in 0..128 {
        match owner.drive_one(heap, account, &[])? {
            LcGraphEvent::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                if node == output {
                    delivered = true;
                    assert!(owner.result(output, heap, account).is_err());
                }
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            LcGraphEvent::Control => {}
            LcGraphEvent::Quiescent => break,
            other => return Err(format!("cancelled dependency delivery: {other:?}").into()),
        }
    }
    assert!(delivered);
    Ok(())
}
