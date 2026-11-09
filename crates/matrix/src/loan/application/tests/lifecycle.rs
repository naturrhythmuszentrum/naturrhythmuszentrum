// SPDX-License-Identifier: AGPL-3.0-only
//! Source reclamation, complete rollback and every-state abandonment retain exact authority.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationPublication, ApplicationRequest,
    HostChannel, LoanError, ProducerOutcome, Residence,
};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Complete scans reclaim only owners whose explicit use edges have ended.
fn reconcile(heap: &mut crate::Heap, account: &mut crate::ExecutionAccount) {
    assert!(matches!(
        heap.reconcile_detached(account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
}
/// The result has only historical source identities after the sole transferred edge ends.
fn source_reclamation<P: Scalar + 'static>() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<P>(3, 1, 3, 2, false, Limits::default())?;
    let source = request.input.ticket()?;
    let demand = request.demand;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    heap.retire_mask(demand, &mut account.ledger)?;
    let owner = candidate(owner, &mut account)?;
    let (tile, receipt) = published(heap.publish_application(owner, &mut account))?;
    reconcile(&mut heap, &mut account);
    let format = crate::storage::Format::of::<P>().index();
    assert_eq!(heap.loans.elements()?[format], 9);
    assert!(matches!(
        heap.acquire_pin::<P>(source, &mut account),
        Err(LoanError::StaleOccupation)
    ));
    assert!(
        heap.loans
            .debits
            .iter()
            .flatten()
            .all(|debit| debit.owner != source.identity)
    );
    let current = tile.reader(&heap, &mut account)?;
    assert!(matches!(
        current.application_outcome(0, &mut account)?,
        crate::CellOutcome::Completed
    ));
    assert_eq!(
        current.read_component(HostChannel::Center, 0, 0, &mut account)?,
        (P::ONE + P::ONE) * (P::ONE + P::ONE)
    );
    let bundle = tile
        .pin()
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("result bundle")?;
    let Some(evidence) = &bundle.origin.evidence else {
        return Err("origin".into());
    };
    assert_eq!(evidence.source_identity, source.identity);
    let additional = heap.acquire_pin::<P>(receipt.ticket(), &mut account)?;
    assert_eq!(additional.ticket()?, receipt.ticket());
    heap.release_pin(additional, &mut account)
        .map_err(|(error, _)| error)?;
    reconcile(&mut heap, &mut account);
    let before = heap.resource_bytes()?;
    reconcile(&mut heap, &mut account);
    assert_eq!(heap.resource_bytes()?, before);
    assert_eq!(heap.loans.elements()?[format], 9);
    drop(tile);
    reconcile(&mut heap, &mut account);
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        Residence::Empty { .. }
    ));
    assert_eq!(heap.loans.elements()?, [0, 0]);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    assert_eq!(
        heap.resource_bytes()?,
        super::bytes::vacant_after_single_application()
    );
    assert!(matches!(
        heap.acquire_pin::<P>(receipt.ticket(), &mut account),
        Err(LoanError::StaleOccupation)
    ));
    Ok(())
}
/// Current result readers and ticket acquisition survive complete source retirement in both formats.
/// ´test:unit:pointwise-result-survives-source-reclamation´
#[test]
fn pointwise_result_survives_source_reclamation() -> TestResult {
    source_reclamation::<f32>()?;
    source_reclamation::<f64>()
}
/// Every actual new allocation boundary closes only the acquired private prefix.
fn admission_prefixes<P: Scalar + 'static>() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<P>(2, 2, 3, 2, false, Limits::default())?;
    let injection = crate::loan::fault::Injection::after(usize::MAX);
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    let count = crate::loan::fault::Injection::attempts();
    assert_eq!(count, 30);
    drop(injection);
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    for prefix in 0..count {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture::<P>(2, 2, 3, 2, false, Limits::default())?;
        let ticket = request.input.ticket()?;
        let pointer = std::rc::Rc::as_ptr(&request.input.record);
        let before = heap.loans.elements()?;
        let bytes = heap.resource_bytes()?;
        let work = account.usage().work;
        let injection = crate::loan::fault::Injection::after(prefix);
        let request = match heap.request_pointwise_square(request, &mut account) {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(
                    LoanError::AllocationRefused | LoanError::Heap(crate::HeapError::Capacity),
                ),
                request,
            ) => request,
            other => return Err(format!("prefix {prefix}: {other:?}").into()),
        };
        assert_eq!(request.input.ticket()?, ticket);
        assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
        assert_eq!(heap.loans.elements()?, before);
        assert_eq!(heap.loans.pending, 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), prefix + 1);
        // Fixed admission 245 plus loops 20+3+10+2: all preparation is prepaid.
        assert_eq!(account.usage().work - work, 280);
        super::prefixes::check(&heap, prefix, bytes)?;
        drop(injection);
        // A complete retry uses the same request and its original single edge.
        let owner = ready(heap.request_pointwise_square(request, &mut account))?;
        let remaining = account.limits().work - account.usage().work;
        account.ledger.work(remaining, crate::loan::AT)?;
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            ApplicationCancellation::ApplicationCancelled(_)
        ));
        assert_eq!(heap.loans.elements()?, before);
    }
    Ok(())
}
/// Every admission prefix returns the original pin and leaves no producer or output prefix.
/// ´test:unit:pointwise-all-admission-allocation-prefixes-clean-up´
#[test]
fn pointwise_all_admission_allocation_prefixes_clean_up() -> TestResult {
    admission_prefixes::<f32>()?;
    admission_prefixes::<f64>()
}
/// The late mask shell is the sole new publication allocation and never reruns evaluation.
/// ´test:unit:pointwise-publication-shell-repair-and-zero-work-cancel´
#[test]
fn pointwise_publication_shell_repair_and_zero_work_cancel() -> TestResult {
    for cancel in [false, true] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture::<f64>(2, 2, 3, 2, false, Limits::default())?;
        let owner = candidate(
            ready(heap.request_pointwise_square(request, &mut account))?,
            &mut account,
        )?;
        let ticket = owner.ticket();
        let pointer = std::ptr::from_ref(owner.core.as_ref());
        let source = owner.core.input.as_ref().ok_or("input")?.ticket()?;
        let injection = crate::loan::fault::Injection::after(0);
        crate::loan::fault::reset_scalars();
        let owner = match heap.publish_application(owner, &mut account) {
            ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::AllocationRefused),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(crate::loan::fault::Injection::attempts(), 1);
        assert_eq!(crate::loan::fault::scalars(), 0);
        drop(injection);
        assert_eq!(owner.ticket(), ticket);
        assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
        assert_eq!(owner.core.input.as_ref().ok_or("input")?.ticket()?, source);
        if cancel {
            let remaining = account.limits().work - account.usage().work;
            account.ledger.work(remaining, crate::loan::AT)?;
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        } else {
            let (tile, _) = published(heap.publish_application(owner, &mut account))?;
            assert_eq!(crate::loan::fault::scalars(), 0);
            assert_eq!(
                tile.reader(&heap, &mut account)?.read_component(
                    HostChannel::Center,
                    0,
                    0,
                    &mut account
                )?,
                4.0
            );
        }
    }
    Ok(())
}
/// Ready, candidate and repair owners all release their one edge before final witness expiration.
/// ´test:unit:pointwise-all-phases-drop-and-unwind-abandon-once´
#[test]
fn pointwise_all_phases_drop_and_unwind_abandon_once() -> TestResult {
    for phase in 0..3 {
        for unwind in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
            let extra = heap.retain_pin(&request.input, &mut account)?;
            let mut owner = ready(heap.request_pointwise_square(request, &mut account))?;
            if phase > 0 {
                owner = candidate(owner, &mut account)?;
            }
            if phase == 2 {
                let injection = crate::loan::fault::Injection::after(0);
                owner = match heap.publish_application(owner, &mut account) {
                    ApplicationPublication::ApplicationRepair(_, owner) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            let ticket = owner.ticket();
            let witness = std::rc::Rc::downgrade(&owner.core.host.witness);
            let input = std::rc::Rc::downgrade(&owner.core.input.as_ref().ok_or("input")?.record);
            crate::loan::fault::watch_source_release(ticket.identity.receipt, input.clone());
            if unwind {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _owner = owner;
                    panic!("exercise application unwind");
                }));
                assert!(result.is_err());
            } else {
                drop(owner);
            }
            assert_eq!(crate::loan::fault::source_at_witness(), Some(0));
            assert_eq!(input.strong_count(), 0);
            assert_eq!(witness.strong_count(), 0);
            assert!(matches!(
                heap.reconcile_detached(&mut account, 100_000, None),
                crate::ScanOutcome::Complete
            ));
            assert!(
                matches!(heap.take_producer_outcome(ticket, &mut account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
            );
            assert_eq!(
                heap.take_producer_outcome(ticket, &mut account)?,
                ProducerOutcome::AlreadyDelivered
            );
            assert_eq!(
                extra.historical_reader(&mut account)?.read_component(
                    HostChannel::Center,
                    0,
                    0,
                    &mut account
                )?,
                2.0
            );
            drop(extra);
            reconcile(&mut heap, &mut account);
            assert_eq!(heap.loans.elements()?, [0, 0]);
        }
    }
    Ok(())
}
