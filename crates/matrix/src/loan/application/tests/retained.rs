// SPDX-License-Identifier: AGPL-3.0-only
//! Private law attachments exercise application ownership without adding an expm1 law arm.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::retained_expm1::tests as laws;
use crate::{
    ApplicationCancellation, ApplicationPublication, HostChannel, LoanError, Residence, ScanOutcome,
};

/// Ready, candidate and repair wrappers preserve the same funded law use and source pin.
#[test]
fn retained_law_application_states_and_source_release() -> TestResult {
    for state in 0..3 {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture::<f64>(1, 1, 1, 2, false, laws::limits())?;
        let source = request.input.ticket()?;
        let demand = request.demand;
        let external = laws::prepare(&mut heap, &mut account)?;
        let moved = heap.retain_retained_expm1(&external, &mut account)?;
        let identity = heap
            .retained_expm1_inventory(&moved, &mut account)?
            .use_identity;
        let mut owner = ready(heap.request_pointwise_square(request, &mut account))?;
        owner.core.host.retained_law = Some(laws::attach(moved, &heap, &mut account)?);
        heap.retire_mask(demand, &mut account.ledger)?;
        drop(external);
        if state != 0 {
            owner = candidate(owner, &mut account)?;
        }
        if state == 2 {
            let injection = crate::loan::fault::Injection::after(0);
            owner = match heap.publish_application(owner, &mut account) {
                ApplicationPublication::ApplicationRepair(
                    crate::ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
            drop(injection);
        }
        laws::check_attachment(
            owner.core.host.retained_law.as_ref().ok_or("law")?,
            identity,
        )?;
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            ApplicationCancellation::ApplicationCancelled(_)
        ));
        assert!(matches!(
            heap.reconcile_detached(&mut account, 1000, None),
            ScanOutcome::Complete
        ));
        assert!(matches!(
            heap.reclaim(&mut account, 1000, None),
            ScanOutcome::Complete
        ));
        assert!(matches!(
            heap.acquire_pin::<f64>(source, &mut account),
            Err(LoanError::StaleOccupation)
        ));
        assert!(matches!(
            heap.residence(&mut account, 1000)?,
            Residence::Empty { .. }
        ));
    }
    Ok(())
}
/// Retiring every numerical source owner cannot release the result's distinct law edge.
#[test]
fn retained_law_result_outlives_numerical_source() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(1, 1, 1, 2, false, laws::limits())?;
    let source = request.input.ticket()?;
    let demand = request.demand;
    let moved = laws::prepare(&mut heap, &mut account)?;
    let identity = heap
        .retained_expm1_inventory(&moved, &mut account)?
        .use_identity;
    let mut owner = ready(heap.request_pointwise_square(request, &mut account))?;
    owner.core.host.retained_law = Some(laws::attach(moved, &heap, &mut account)?);
    heap.retire_mask(demand, &mut account.ledger)?;
    let (tile, receipt) =
        published(heap.publish_application(candidate(owner, &mut account)?, &mut account))?;
    let pin = heap.retain_pin(tile.pin(), &mut account)?;
    drop(tile);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.acquire_pin::<f64>(source, &mut account),
        Err(LoanError::StaleOccupation)
    ));
    let bundle = pin
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("bundle")?;
    laws::check_attachment(bundle.retained_law.as_ref().ok_or("law")?, identity)?;
    assert_eq!(
        pin.historical_reader(&mut account)?.read_component(
            HostChannel::Center,
            0,
            0,
            &mut account
        )?,
        4.0
    );
    let current = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
    drop(current);
    drop(heap);
    let bundle = pin
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("bundle")?;
    laws::check_attachment(bundle.retained_law.as_ref().ok_or("law")?, identity)?;
    Ok(())
}

/// Every detached application phase releases its same law edge on ordinary drop and unwind.
#[test]
fn retained_law_application_drop_and_unwind() -> TestResult {
    for state in 0..3 {
        for unwind in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(1, 1, 1, 2, false, laws::limits())?;
            let moved = laws::prepare(&mut heap, &mut account)?;
            let identity = heap
                .retained_expm1_inventory(&moved, &mut account)?
                .use_identity;
            let mut owner = ready(heap.request_pointwise_square(request, &mut account))?;
            owner.core.host.retained_law = Some(laws::attach(moved, &heap, &mut account)?);
            if state != 0 {
                owner = candidate(owner, &mut account)?;
            }
            if state == 2 {
                let injection = crate::loan::fault::Injection::after(0);
                owner = match heap.publish_application(owner, &mut account) {
                    ApplicationPublication::ApplicationRepair(_, owner) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            let destroyed = laws::destroyed();
            drop(heap);
            assert_eq!(laws::destroyed(), destroyed);
            laws::check_attachment(
                owner.core.host.retained_law.as_ref().ok_or("law")?,
                identity,
            )?;
            let work = account.usage().work;
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("application law unwind fixture");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(laws::destroyed(), destroyed + 1);
            assert_eq!(account.usage().work, work);
        }
    }
    Ok(())
}
