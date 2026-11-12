// SPDX-License-Identifier: AGPL-3.0-only
//! Every ancestor is an eligibility dependency, independently of sibling and immediate pin lifetimes.

use super::super::super::ready;
use super::{TestResult, admit, candidate, prefix, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationPublication, ApplicationRequest,
    Elementwise, HostChannel, LoanError,
};

/// Retire each host and the intermediate binary ancestor separately, before admission or publication.
#[test]
fn chain_each_unpinned_ancestor_retires_independently() -> TestResult {
    for reciprocal in [false, true] {
        for target in 0..if reciprocal { 3 } else { 2 } {
            for publication in [false, true] {
                retire(reciprocal, target, publication)?;
            }
        }
    }
    Ok(())
}

/// Explicit pins preserve all other ancestors while one requested occupation is reclaimed.
fn retire(reciprocal: bool, target: usize, publication: bool) -> TestResult {
    let (mut heap, mut account, tile, demands) = prefix(1, reciprocal)?;
    let immediate = tile.pin().ticket()?;
    let ancestors = heap
        .loans
        .groups
        .iter()
        .flatten()
        .filter(|group| group.identity != immediate.identity)
        .map(|group| crate::LoanTicket {
            identity: group.identity,
        })
        .collect::<Vec<_>>();
    assert_eq!(ancestors.len(), if reciprocal { 3 } else { 2 });
    let mut kept = Vec::new();
    for (index, ticket) in ancestors.iter().copied().enumerate() {
        if index != target {
            kept.push(heap.acquire_pin::<f64>(ticket, &mut account)?);
        }
    }
    let extra = heap.retain_pin(tile.pin(), &mut account)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
    let (request, owner) = if publication {
        (
            None,
            Some(candidate(
                ready(admit(&mut heap, request, function, &mut account))?,
                &mut account,
            )?),
        )
    } else {
        (Some(request), None)
    };
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
    }
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(heap.loans.group(ancestors[target].identity).is_err());
    assert!(heap.loans.group(immediate.identity).is_ok());
    let pin = heap.acquire_pin::<f64>(immediate, &mut account)?;
    assert!(
        pin.historical_reader(&mut account)?
            .read_component(HostChannel::Center, 0, 0, &mut account)?
            .is_finite()
    );
    finish_stale(&mut heap, &mut account, request, owner, function)?;
    drop(heap);
    assert!(
        extra
            .historical_reader(&mut account)?
            .read_component(HostChannel::Center, 0, 0, &mut account)?
            .is_finite()
    );
    drop(kept);
    Ok(())
}

/// A pending, cancelled or abandoned producer cannot supply any input capability to the new route.
#[test]
fn chain_pending_cancelled_abandoned_producers_cannot_supply_pins() -> TestResult {
    for state in 0..3 {
        let (mut heap, mut account, tile, _) = super::product(1, 1)?;
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        let owner = ready(admit(
            &mut heap,
            request,
            Elementwise::SquareRoot,
            &mut account,
        ))?;
        let ticket = owner.ticket();
        assert!(matches!(
            heap.acquire_pin::<f64>(ticket, &mut account),
            Err(LoanError::Heap(crate::HeapError::OwnershipPending))
        ));
        if state == 0 {
            drop(owner);
        } else if state == 1 {
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        } else {
            drop(owner);
            assert!(matches!(
                heap.reconcile_detached(&mut account, 100_000, None),
                crate::ScanOutcome::Complete
            ));
        }
        assert!(heap.acquire_pin::<f64>(ticket, &mut account).is_err());
    }
    Ok(())
}

/// Stale evidence remains stale on retry; cancellation uses the existing prepaid entitlement.
fn finish_stale(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    request: Option<crate::PointwiseRequest<f64>>,
    owner: Option<crate::OwnedApplication<f64>>,
    function: Elementwise,
) -> TestResult {
    if let Some(request) = request {
        assert!(matches!(
            admit(heap, request, function, account),
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::StaleDependency),
                _
            )
        ));
    }
    if let Some(mut owner) = owner {
        for _ in 0..2 {
            owner = match heap.publish_application(owner, account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::StaleDependency),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
        }
        assert!(matches!(
            heap.cancel_application(owner, account),
            ApplicationCancellation::ApplicationCancelled(_)
        ));
    }
    Ok(())
}
