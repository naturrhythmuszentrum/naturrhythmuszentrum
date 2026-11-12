// SPDX-License-Identifier: AGPL-3.0-only
//! Ancestors provide eligibility while only the immediate input has a retaining edge.

use super::{TestResult, admit, candidate, prefix, product, published, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationPublication, ApplicationRequest,
    Elementwise, HostChannel, LoanError, ScanOutcome,
};

/// Both bounded passes use the original ledger; demand retirement is a distinct caller operation.
fn collect(heap: &mut crate::Heap, account: &mut crate::ExecutionAccount) {
    assert!(matches!(
        heap.reconcile_detached(account, 100_000, None),
        ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(account, 100_000, None),
        ScanOutcome::Complete
    ));
}

/// A retained result remains current and historically readable after its unpinned lineage retires.
/// ´test:unit:chain-stale-lineage-does-not-revoke-reading´
#[test]
fn chain_stale_lineage_does_not_revoke_reading() -> TestResult {
    for reciprocal in [false, true] {
        let (mut heap, mut account, tile, demands) = prefix(1, reciprocal)?;
        for demand in demands {
            heap.retire_mask(demand, &mut account.ledger)?;
        }
        collect(&mut heap, &mut account);
        let center = tile.reader(&heap, &mut account)?.read_component(
            HostChannel::Center,
            0,
            0,
            &mut account,
        )?;
        let function = if reciprocal {
            Elementwise::Reciprocal
        } else {
            Elementwise::SquareRoot
        };
        let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
        let pin = std::rc::Rc::as_ptr(&request.input.record);
        let before = heap.resource_bytes()?;
        let injection = crate::loan::fault::Injection::after(0);
        let request = match admit(&mut heap, request, function, &mut account) {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::StaleDependency),
                request,
            ) => request,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pin);
        assert_eq!(heap.resource_bytes()?, before);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
        drop(heap);
        assert_eq!(
            request
                .input
                .historical_reader(&mut account)?
                .read_component(HostChannel::Center, 0, 0, &mut account)?
                .to_bits(),
            center.to_bits()
        );
    }
    Ok(())
}

/// Reclamation cannot remove the immediate source; a stale candidate stays repairable only by cancellation.
#[test]
fn chain_stale_publication_repeats_without_numerical_access() -> TestResult {
    let (mut heap, mut account, tile, demand) = product(3, 1)?;
    let source = tile.pin().ticket()?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let owner = super::super::super::ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    let owner = candidate(owner, &mut account)?;
    let pointer = std::ptr::from_ref(owner.core.as_ref());
    heap.retire_mask(demand, &mut account.ledger)?;
    collect(&mut heap, &mut account);
    assert!(heap.loans.group(source.identity).is_ok());
    let mut owner = owner;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    for _ in 0..2 {
        owner = match heap.publish_application(owner, &mut account) {
            ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::StaleDependency),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
        assert!(heap.loans.group(source.identity).is_ok());
    }
    assert_eq!(
        [crate::loan::fault::reads(), crate::loan::fault::scalars()],
        [0; 2]
    );
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    Ok(())
}

/// Detached evaluation needs only its immediate pin even when the heap and every ancestor disappear.
#[test]
fn chain_evaluation_after_heap_destruction() -> TestResult {
    let (mut heap, mut account, tile, _) = product(3, 1)?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let owner = super::super::super::ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    drop(heap);
    let owner = candidate(owner, &mut account)?;
    assert!(
        owner
            .core
            .evidence
            .outcomes
            .iter()
            .all(|outcome| matches!(outcome, crate::CellOutcome::Completed))
    );
    Ok(())
}

/// 11 groups + 5 pins + 44 debits + 5 expired-pin checks = 65; root passes cost 98.
/// Vacant residence is 48+11*(256+1032)+13*32+44*264+5*40 = 26448 bytes.
/// ´test:unit:chain-canonical-cleanup-work-and-vacant-residence´
#[test]
fn chain_canonical_cleanup_work_and_vacant_residence() -> TestResult {
    let (mut heap, mut account, tile, mut demands) = prefix(1, true)?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut account,
    )?;
    demands.push(request.demand);
    let owner = super::super::super::ready(admit(
        &mut heap,
        request,
        Elementwise::Reciprocal,
        &mut account,
    ))?;
    let owner = candidate(owner, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
    }
    drop(tile);
    let before = account.usage().work;
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(account.usage().work - before, 65);
    let before = account.usage().work;
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(account.usage().work - before, 98);
    assert_eq!(heap.resource_bytes()?, 26_448);
    assert_eq!(heap.loans.elements()?, [0; 2]);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    collect(&mut heap, &mut account);
    assert_eq!(heap.resource_bytes()?, 26_448);
    Ok(())
}
