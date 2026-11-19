// SPDX-License-Identifier: AGPL-3.0-only
//! Extra uses retain complete historical owners; weak debit expiration never releases live payload.
use super::{Scalar, TestResult, demand, request, setup};
use crate::{HostChannel, LcOmegaOutput, LoanError};
use nrz_bindings::Limits;

/// Preserve numerical history through source facade retirement, independent use release and heap drop.
pub(in crate::loan::lc_shape::tests::output) fn history<P: Scalar + 'static>() -> TestResult {
    for heap_first in [false, true] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            2,
            1,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|f| f.into_parts().1)?;
        let closure = std::rc::Rc::downgrade(&prepared.receipt.closure);
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let candidate = pending
            .evaluate(&mut s.account)
            .map_err(super::super::execution::failure)?;
        let result = candidate
            .publish(&mut s.heap, &mut s.account)
            .map_err(|f| f.error)?;
        let ticket = result.ticket();
        let layouts = members.detach();
        let value = result.historical_reader(&mut s.account)?.read(
            HostChannel::Center,
            0,
            &mut s.account,
        )?;
        let mut foreign =
            crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
        let before = foreign.usage().work;
        assert!(matches!(
            result.historical_reader(&mut foreign),
            Err(LoanError::ForeignAccount)
        ));
        assert!(matches!(
            result.retain(&mut s.heap, &mut foreign),
            Err(LoanError::ForeignAccount)
        ));
        let (result, error) = result
            .release(&mut foreign)
            .expect_err("foreign use release");
        assert!(matches!(error, LoanError::ForeignAccount));
        assert_eq!(foreign.usage().work, before);
        let retained = result.retain(&mut s.heap, &mut s.account)?;
        assert_eq!(retained.ticket(), ticket);
        assert_eq!(closure.strong_count(), 1);
        result.release(&mut s.account).map_err(|(_, error)| error)?;
        if heap_first {
            assert!(matches!(
                s.heap.take_producer_outcome(ticket, &mut s.account)?,
                crate::ProducerOutcome::AlreadyDelivered
            ));
            drop(s.heap);
            let reader = retained.historical_reader(&mut s.account)?;
            assert_eq!(
                reader
                    .read(HostChannel::Center, 0, &mut s.account)?
                    .to_f64()
                    .to_bits(),
                value.to_f64().to_bits()
            );
            let consume = s.account.limits().work - s.account.usage().work;
            s.account.ledger.work(consume, crate::loan::AT)?;
            retained
                .release(&mut s.account)
                .map_err(|(_, error)| error)?;
        } else {
            live_heap(retained, &mut s.heap, &mut s.account, mask, layouts, value)?;
        }
        assert_eq!(closure.strong_count(), 0);
    }
    Ok(())
}

/// Real source retirement is permanent for this captured candidate; retry never reruns arithmetic.
pub(in crate::loan::lc_shape::tests::output) fn stale<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        2,
        1,
    )?;
    let prepared = output
        .prepare(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &mut s.account,
        )
        .map_err(|f| f.into_parts().1)?;
    // Retain every original except the final unused role. Its retirement alone must refuse publication.
    let protected = protect_selected(&prepared, &mut s.heap, &mut s.account)?;
    let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
    let pending = prepared
        .admit(&mut s.heap, mask, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let mut candidate = pending
        .evaluate(&mut s.account)
        .map_err(super::super::execution::failure)?;
    let ticket = candidate.ticket();
    let mut wrong = crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
    let before_wrong = wrong.usage().work;
    let refusal = candidate
        .publish(&mut s.heap, &mut wrong)
        .expect_err("foreign publication account");
    assert!(matches!(refusal.error, LoanError::ForeignAccount));
    assert_eq!(wrong.usage().work, before_wrong);
    candidate = refusal.owner;
    let mut foreign = crate::Heap::new(crate::HeapLimits::default())?;
    let before = s.account.usage().work;
    let failure = candidate
        .publish(&mut foreign, &mut s.account)
        .expect_err("foreign heap");
    assert!(matches!(failure.error, LoanError::ForeignOwner));
    assert_eq!(s.account.usage().work, before);
    candidate = failure.owner;
    let layouts = members.detach();
    assert!(matches!(
        s.heap.reclaim(&mut s.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    for _ in 0..2 {
        let before = s.account.usage().work;
        let failure = candidate
            .publish(&mut s.heap, &mut s.account)
            .expect_err("retired captured source");
        assert!(matches!(failure.error, LoanError::StaleOccupation));
        assert_eq!(s.account.usage().work - before, 639);
        assert_eq!(failure.owner.ticket(), ticket);
        candidate = failure.owner;
    }
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    candidate.cancel(&mut s.heap, &mut s.account)?;
    drop(candidate);
    drop(protected);
    s.heap.retire_mask(mask, &mut s.account.ledger)?;
    super::super::fixture::retire_layout(&mut s.heap, &mut s.account, mask.layout())?;
    for layout in layouts {
        super::super::fixture::retire_layout(&mut s.heap, &mut s.account, layout)?;
    }
    super::super::fixture::empty(&mut s.heap, &mut s.account)
}

/// Registry-only reclamation waits for the final specialized use before caller roots retire.
fn live_heap<P: Scalar + 'static>(
    retained: crate::LcPublishedResult<P>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    mask: crate::MaskVersion,
    layouts: Vec<crate::CellLayoutVersion>,
    value: P,
) -> TestResult {
    assert!(matches!(
        heap.reconcile_detached(account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let reader = retained.current_reader(heap, account)?;
    assert_eq!(
        reader
            .read(HostChannel::Center, 0, account)?
            .to_f64()
            .to_bits(),
        value.to_f64().to_bits()
    );
    retained.release(account).map_err(|(_, error)| error)?;
    heap.residence(account, 100_000)?;
    heap.retire_mask(mask, &mut account.ledger)?;
    super::super::fixture::retire_layout(heap, account, mask.layout())?;
    for layout in layouts {
        super::super::fixture::retire_layout(heap, account, layout)?;
    }
    super::super::fixture::empty(heap, account)?;
    Ok(())
}

/// Keep selected and early unused originals live while the final unused original retires.
fn protect_selected<P: Scalar + 'static>(
    prepared: &crate::LcOmegaPrepared<'_, P, 5>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<Vec<crate::BundlePin<P>>> {
    Ok(prepared
        .receipt
        .closure
        .rows
        .iter()
        .take(4)
        .map(|row| {
            heap.acquire_pin::<P>(
                crate::LoanTicket {
                    identity: row.source,
                },
                account,
            )
        })
        .collect::<Result<Vec<_>, _>>()?)
}
