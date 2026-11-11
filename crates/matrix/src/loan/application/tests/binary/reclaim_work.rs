// SPDX-License-Identifier: AGPL-3.0-only
//! Independent scan and owner/token visit counts fund binary reclamation before removal.

use super::{
    BinaryElementwise, Limits, TestResult, admit, candidate, fixture, leave, published, ready,
};
use crate::loan::fault;
use crate::{ExecutionAccount, Heap, LoanTicket, ScanCursor, ScanOutcome};

/// Publish through the public lifecycle, retire demand and reconcile every expired pin index.
fn expired(function: BinaryElementwise) -> TestResult<(Heap, ExecutionAccount, LoanTicket)> {
    let mut setup = fixture(1, 1, [1; 2], 2, false, function, Limits::default())?;
    let demand = setup.request.demand;
    let owner = candidate(
        ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?,
        &mut setup.account,
    )?;
    let (tile, receipt) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    drop(tile);
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 1000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(setup.heap.loans.groups.len(), 5);
    assert!(setup.heap.loans.pins.iter().all(Option::is_none));
    assert_eq!(receipt.ticket().identity.root.slot, 2);
    Ok((setup.heap, setup.account, receipt.ticket()))
}

/// The public diagnostic view pins the stopped position and whether another pass is required.
fn cursor(stopped: ScanCursor, heap: &Heap, next: usize, changed: bool) {
    assert_eq!(
        format!("{stopped:?}"),
        format!(
            "ScanCursor {{ scope: {}, revision: {}, reclaim: true, next: {next}, end: 5, changed: {changed} }}",
            heap.scope, heap.loans.revision,
        )
    );
}

/// Two five-row passes cost ten visits; each source adds one root, six owner/token and five removals.
/// The result adds one root, ten owner/token and seven removals: 10 + 2*12 + 18 = 52.
#[test]
fn binary_reclaim_complete_pass_literal_work_neighbours() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for available in [52, 51] {
            let (mut heap, mut account, _) = expired(function)?;
            let limits = account.limits().clone();
            leave(&mut account, available)?;
            let before = account.usage().work;
            match heap.reclaim(&mut account, 1000, None) {
                ScanOutcome::Complete => assert_eq!(available, 52),
                ScanOutcome::WorkExhausted(stopped) => {
                    assert_eq!(available, 51);
                    cursor(stopped, &heap, 4, false);
                }
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(account.usage().work - before, available);
            assert_eq!(account.limits(), &limits);
            assert!(heap.loans.groups.iter().all(Option::is_none));
            assert!(heap.loans.debits.iter().all(Option::is_none));
            assert_eq!(heap.loans.elements()?, [0; 2]);
        }
    }
    Ok(())
}

/// The result needs one slot, one root, ten validations and seven removals: 19, never 18.
/// Validation covers three retained tokens, three channel tokens, two evidence tokens and two owners.
#[test]
fn binary_reclaim_result_root_literal_work_neighbours() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for available in [19, 18] {
            let (mut heap, mut account, ticket) = expired(function)?;
            let ScanOutcome::Pending(stopped) = heap.reclaim(&mut account, 2, None) else {
                return Err("two source removals must leave the result pending".into());
            };
            cursor(stopped, &heap, 2, true);
            assert!(heap.loans.groups[..2].iter().all(Option::is_none));
            let released = fault::numerical_releases();
            let bytes = heap.resource_bytes()?;
            let limits = account.limits().clone();
            leave(&mut account, available)?;
            let before = account.usage().work;
            match heap.reclaim(&mut account, 1, Some(stopped)) {
                ScanOutcome::Pending(stopped) => {
                    assert_eq!(available, 19);
                    cursor(stopped, &heap, 3, true);
                }
                ScanOutcome::WorkExhausted(stopped) => {
                    assert_eq!(available, 18);
                    cursor(stopped, &heap, 2, true);
                }
                other => return Err(format!("{other:?}").into()),
            }
            let removed = available == 19;
            assert_eq!(account.usage().work - before, if removed { 19 } else { 12 });
            assert_eq!(heap.loans.groups[2].is_none(), removed);
            assert_eq!(
                heap.descriptors
                    .slots
                    .at(ticket.identity.root.slot)
                    .ok_or("root")?
                    .1
                    .is_none(),
                removed
            );
            let slabs = usize::from(removed) * 3;
            assert_eq!(
                fault::numerical_releases(),
                [
                    released[0] + slabs,
                    released[1] + slabs,
                    released[2] + 64 * slabs
                ]
            );
            if removed {
                assert!(heap.resource_bytes()? < bytes);
                assert!(heap.loans.debits.iter().all(Option::is_none));
            } else {
                assert_eq!(heap.resource_bytes()?, bytes);
            }
            assert_eq!(account.limits(), &limits);
        }
    }
    Ok(())
}
