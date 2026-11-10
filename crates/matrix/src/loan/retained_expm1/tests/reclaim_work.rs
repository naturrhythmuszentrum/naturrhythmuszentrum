// SPDX-License-Identifier: AGPL-3.0-only
//! Published roots pay law validation before removal; expired law debits need later visits.

use super::{DESTROYED, TestResult, attach, fixture, host, publish};
use crate::{ExecutionAccount, Heap, HeapLimits, Residence, ScanOutcome};

/// Assert the stopped pass without opening the cursor's production visibility.
fn cursor(cursor: crate::ScanCursor, heap: &Heap, next: usize, changed: bool) {
    // Debug is the public diagnostic view of the otherwise opaque resumable cursor.
    assert_eq!(
        format!("{cursor:?}"),
        format!(
            "ScanCursor {{ scope: {}, revision: {}, reclaim: true, next: {next}, end: 1, changed: {changed} }}",
            heap.scope, heap.loans.revision,
        )
    );
}

/// Reclaim has destroyed the payload, but each weak law debit needs its own charged removal.
fn reconcile_law(heap: &mut Heap, account: &mut ExecutionAccount) -> TestResult {
    assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
    assert!(heap.loans.debits[0].is_some());
    assert!(heap.loans.debits[1].is_some());
    let bytes = heap.resource_bytes()?;
    assert!(matches!(
        heap.residence(account, 0)?,
        Residence::Pending { .. }
    ));
    assert_eq!(heap.resource_bytes()?, bytes);
    let before = account.usage().work;
    // Vacant group, vacant public pin, then owner visit and authenticated index removal.
    let ScanOutcome::Pending(stopped) = heap.reconcile_detached(account, 3, None) else {
        return Err("law use must remain pending after owner reconciliation".into());
    };
    assert_eq!(account.usage().work - before, 1 + 1 + 1 + 1);
    assert!(heap.loans.debits[0].is_none());
    assert!(heap.loans.debits[1].is_some());
    assert!(heap.resource_bytes()? < bytes);
    assert!(matches!(
        heap.residence(account, 0)?,
        Residence::Pending { .. }
    ));
    let before = account.usage().work;
    let result = heap.reconcile_detached(account, 1, Some(stopped));
    assert!(matches!(
        result,
        ScanOutcome::Pending(_) | ScanOutcome::Complete
    ));
    assert_eq!(account.usage().work - before, 1 + 1);
    assert_eq!(heap.loans.debits.iter().flatten().count(), 0);
    assert!(matches!(
        heap.residence(account, 1000)?,
        Residence::Empty { .. }
    ));
    Ok(())
}

/// One channel: scan + lookup + (channel, five bundle checks, three law checks) + five edges.
/// The unchanged vacancy pass adds one visit: 17/16 complete-pass and 16/15 removal controls.
#[test]
fn retained_law_published_reclaim_work_boundaries() -> TestResult {
    let scan = 1;
    let lookup = 1;
    let validation = 1 + 5 + 3;
    let removals = 1 + 4;
    let root_work = scan + lookup + validation + removals;
    let pass_work = root_work + 1;
    assert_eq!((root_work, pass_work), (16, 17));
    for complete in [false, true] {
        for short in [0, 1] {
            reclaim_boundary(complete, short)?;
        }
    }
    Ok(())
}

/// Each twin uses its own original fixed account and inspects the stopped root before cleanup.
fn reclaim_boundary(complete: bool, short: u64) -> TestResult {
    let destroyed = DESTROYED.get();
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let limits = account.limits().clone();
    let law = fixture::prepare(&mut heap, &mut account)?;
    let mut loan = host(&mut heap, &mut account)?;
    attach(&heap, &mut loan, law, &mut account)?;
    let (tile, receipt) = publish(&mut heap, loan, &mut account)?;
    let root = receipt.ticket().identity.root;
    drop(tile);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(DESTROYED.get(), destroyed);
    assert_eq!(heap.loans.groups.len(), 1);
    assert_eq!(heap.loans.pins.len(), 1);
    assert_eq!(heap.loans.pins.iter().flatten().count(), 0);
    let root_work = 1 + 1 + (1 + 5 + 3) + (1 + 4);
    let available = root_work + u64::from(complete) - short;
    let prior = fixture::leave_work(&mut account, available)?;
    let allowance = if complete { 1000 } else { 1 };
    let result = heap.reclaim(&mut account, allowance, None);
    let removed = available >= root_work;
    assert_eq!(
        account.usage().work - prior,
        if removed {
            available
        } else {
            1 + 1 + (1 + 5 + 3)
        }
    );
    match result {
        ScanOutcome::Complete => {
            let full_pass = complete && short == 0;
            assert!(full_pass);
        }
        ScanOutcome::Pending(stopped) => {
            let removed_only = !complete && short == 0;
            assert!(removed_only);
            cursor(stopped, &heap, 1, true);
        }
        ScanOutcome::WorkExhausted(stopped) => {
            assert_eq!(short, 1);
            cursor(stopped, &heap, 0, false);
        }
        ScanOutcome::Refused(error) => return Err(error.into()),
    }
    assert_eq!(heap.loans.groups[0].is_none(), removed);
    assert_eq!(
        heap.descriptors
            .slots
            .at(root.slot)
            .ok_or("root slot")?
            .1
            .is_none(),
        removed
    );
    assert_eq!(DESTROYED.get(), destroyed + usize::from(removed));
    assert_eq!(account.limits(), &limits);
    let mut cleanup = fixture::account()?;
    assert!(matches!(
        heap.residence(&mut cleanup, 0)?,
        Residence::Pending { .. }
    ));
    if !removed {
        assert!(matches!(
            heap.reclaim(&mut cleanup, 1000, None),
            ScanOutcome::Complete
        ));
    }
    assert_eq!(DESTROYED.get(), destroyed + 1);
    reconcile_law(&mut heap, &mut cleanup)?;
    Ok(())
}
