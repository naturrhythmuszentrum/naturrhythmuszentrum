// SPDX-License-Identifier: AGPL-3.0-only
//! New input occupation is independent of an old owner's outstanding specialized reader.
use super::{
    super::{
        admission::Refusal,
        drive::Event,
        input::InputRequest,
        node::Node,
        owner::{Capacities, Owner},
        residence::Completion,
    },
    fixture::TestResult,
};
use crate::{ExecutionAccount, Heap, LcInputInventory, LcShapeAxes};
use nrz_bindings::Registry;
use nrz_scalar::Scalar;

/// Both the named owner set and the heap snapshot remain explicit during renewal.
pub(super) fn run<'i, P: Scalar + 'static>(
    old: &Owner<'i, P, 5>,
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let node = Node::input(
        inventory
            .input(0)
            .ok_or("renewed original")?
            .source(axes, account)?,
    );
    let mut new = Owner::admit(
        vec![node],
        &[1],
        Capacities {
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
        &[old.borrowed()],
        heap,
        account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    let handle = new.handle(0, account)?;
    assert_ne!(handle.owner, old.backing.core.reservation.backing.identity);
    let InputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = new.request_input(handle, heap, account, &[old.borrowed()])?
    else {
        return Err("renewed input".into());
    };
    control?;
    new.acknowledge_probe(evidence, account)?;
    assert!(matches!(
        Owner::completion(&[old], heap, account, 100_000)?,
        Completion::NativePending(..)
    ));
    assert!(matches!(
        Owner::completion(&[&new], heap, account, 100_000)?,
        Completion::GraphPending(_)
    ));
    assert!(matches!(
        Owner::completion(&[old, &new, old], heap, account, 100_000)?,
        Completion::GraphPending(_)
    ));
    assert!(heap.loan_inventory(loan.ticket(), account)?.elements > 0);
    let crate::CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, account) else {
        return Err("renewed native cancellation".into());
    };
    new.input_cancelled(handle, receipt, heap, account)?;
    for _ in 0..16 {
        match new.drive_one(heap, account, &[old.borrowed()])? {
            Event::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                new.acknowledge_outcome(node, evidence, heap, account)?;
            }
            Event::Control => {}
            Event::Quiescent => break,
            other => return Err(format!("renewed drain: {other:?}").into()),
        }
    }
    // The old outside reader survives the new owner's complete graph and native input cancellation.
    assert!(matches!(
        Owner::completion(&[old, &new], heap, account, 100_000)?,
        Completion::NativePending(..)
    ));
    Ok(())
}
