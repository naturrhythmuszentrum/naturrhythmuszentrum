// SPDX-License-Identifier: AGPL-3.0-only
//! A new public input owner remains independent while an old sealed reader drains.
use super::fixture::Result;
use nrz_bindings::Registry;
use nrz_matrix::*;
use nrz_scalar::Scalar;
/// Both the named owner set and the heap snapshot remain explicit during renewal.
pub(super) fn run<'i, P: Scalar + 'static>(
    old: &LcGraphOwner<'i, P, 5>,
    original_handle: LcGraphHandle,
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let node = LcGraphNode::input(
        inventory
            .input(0)
            .ok_or("renewed original")?
            .source(axes, account)?,
    );
    let mut new = LcGraphOwner::admit(
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
        &[old.borrowed()],
        heap,
        account,
    )
    .map_err(|LcGraphAdmissionFailure { error, .. }| error)?;
    let handle = new.handle(0, account)?;
    assert_ne!(handle, original_handle);
    let LcGraphInputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = new.request_input(handle, heap, account, &[old.borrowed()])?
    else {
        return Err("renewed input".into());
    };
    control?;
    new.acknowledge_probe(evidence, account)?;
    named(old, &new, heap, account)?;
    assert!(heap.loan_inventory(loan.ticket(), account)?.elements > 0);
    let loan = foreign(&mut new, original_handle, loan, heap, account)?;
    let CancelOutcome::Cancelled(receipt) = heap.cancel_loan(loan, account) else {
        return Err("renewed native cancellation".into());
    };
    new.input_cancelled(handle, receipt, heap, account)?;
    for _ in 0..16 {
        match new.drive_one(heap, account, &[old.borrowed()])? {
            LcGraphEvent::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                new.acknowledge_outcome(node, evidence, heap, account)?;
            }
            LcGraphEvent::Control => {}
            LcGraphEvent::Quiescent => break,
            other => return Err(format!("renewed drain: {other:?}").into()),
        }
    }
    // The old outside reader survives the new owner's complete graph and native input cancellation.
    assert!(matches!(
        LcGraphOwner::completion(&[old, &new], heap, account, 100_000)?,
        LcGraphCompletion::NativePending(..)
    ));
    Ok(())
}

/// A foreign retired occupation cannot consume the new owner's original loan.
fn foreign<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    foreign: LcGraphHandle,
    loan: OwnedLoan<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<OwnedLoan<P>> {
    let ticket = loan.ticket();
    let LcGraphInputReturn::Rejected {
        loan,
        error: LcGraphError::Native(LoanError::ForeignOwner),
    } = owner.return_input(foreign, loan, heap, account)
    else {
        return Err("foreign handback".into());
    };
    assert_eq!(loan.ticket(), ticket);
    Ok(loan)
}

/// Names include aliases in supplied order, while cardinality counts original allocations once.
fn named<P: Scalar + 'static>(
    old: &LcGraphOwner<'_, P, 5>,
    new: &LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let old_set = [old];
    let new_set = [new];
    let combined = [old, new, old];
    let old_result = LcGraphOwner::completion(&old_set, heap, account, 100_000)?;
    let new_result = LcGraphOwner::completion(&new_set, heap, account, 100_000)?;
    let combined_result = LcGraphOwner::completion(&combined, heap, account, 100_000)?;
    assert!(matches!(old_result, LcGraphCompletion::NativePending(..)));
    assert!(matches!(new_result, LcGraphCompletion::GraphPending(_)));
    assert!(matches!(
        combined_result,
        LcGraphCompletion::GraphPending(_)
    ));
    assert_eq!(old_result.owners().distinct(), 1);
    assert_eq!(new_result.owners().distinct(), 1);
    assert_eq!(combined_result.owners().distinct(), 2);
    let old_id = old_result
        .owners()
        .identities()
        .next()
        .ok_or("old owner name")?;
    let new_id = new_result
        .owners()
        .identities()
        .next()
        .ok_or("new owner name")?;
    assert_ne!(old_id, new_id);
    assert_eq!(
        combined_result.owners().identities().collect::<Vec<_>>(),
        [old_id, new_id, old_id]
    );
    Ok(())
}
