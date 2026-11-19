// SPDX-License-Identifier: AGPL-3.0-only
//! Checked pending owns all allocations while exposing only demand and cancellation.
pub(super) mod receipts;
use super::{TestResult, demand, request, setup};
use crate::{LcOmegaOutput, LcOutputCellState, LoanError};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Pending initialization and owner-level cancellation never fabricate filled outcomes.
/// ´test:unit:lc-origin-outcome-partition-encoding´
#[test]
fn lc_origin_outcome_partition_encoding() -> TestResult {
    super::publication::partition::<f32>()?;
    super::publication::partition::<f64>()?;
    super::execution::images::<f32>()?;
    super::execution::images::<f64>()?;
    crate::loan::lc_origin::pending::execution::checks::mapping()?;
    lifecycle::<f32>(false)?;
    lifecycle::<f64>(false)
}

/// Adopted closure identities survive admission; dead pending witnesses reconcile independently.
/// ´test:unit:lc-origin-source-closure-retry-history´
#[test]
fn lc_origin_source_closure_retry_history() -> TestResult {
    super::publication::stale::<f32>()?;
    super::publication::stale::<f64>()?;
    super::publication::history::<f32>()?;
    super::publication::history::<f64>()?;
    super::execution::refusals::<f32>()?;
    super::execution::refusals::<f64>()?;
    heap_first::<f32>()?;
    heap_first::<f64>()?;
    exhausted_drop::<f32>()?;
    exhausted_drop::<f64>()?;
    lifecycle::<f32>(true)?;
    lifecycle::<f64>(true)
}

/// Admission and cancellation preserve the same account and producer identity.
/// ´test:unit:lc-origin-phase-receipts´
#[test]
fn lc_origin_phase_receipts() -> TestResult {
    super::publication::receipts::<f32>()?;
    super::publication::receipts::<f64>()?;
    super::execution::receipts::<f32>()?;
    super::execution::receipts::<f64>()?;
    receipts::check::<f32>()?;
    receipts::check::<f64>()
}

fn lifecycle<P: Scalar + 'static>(abandon: bool) -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let tiles = members.references();
    let prepared = output
        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let selected = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
    let source_pins: Vec<_> = prepared
        .receipt
        .closure
        .rows
        .iter()
        .map(|row| std::rc::Rc::downgrade(&row.pin))
        .collect();
    let watched = std::rc::Rc::downgrade(&prepared.receipt.closure.rows[0].pin);
    let identity = std::rc::Rc::as_ptr(&prepared.receipt.closure);
    let before = s.heap.loans.debits.iter().flatten().count();
    crate::loan::fault::reset_reads();
    crate::loan::fault::take_lc_pair_visits();
    let mut pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    crate::loan::lc_origin::pending::assert_private_demand_0_3_11(&pending);
    assert_eq!(crate::loan::fault::take_lc_pair_visits(), 5 * 40);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(s.heap.loans.debits.iter().flatten().count(), before + 17);
    assert!(s.heap.loans.debits.iter().flatten().any(|debit| {
        matches!(&debit.kind, crate::loan::registry::AllocationKind::LcClosure(witness) if witness.as_ptr() == identity)
    }));
    assert!(std::ptr::eq(pending.inventory(), &raw const s.inventory));
    assert_eq!(
        pending.cell_state(0, &mut s.account)?,
        LcOutputCellState::Pending
    );
    assert_eq!(
        pending.cell_state(1, &mut s.account)?,
        LcOutputCellState::NotRequested
    );
    if abandon {
        crate::loan::lc_origin::pending::reverse_slots(&mut s.heap, &mut pending);
    }
    let ticket = pending.ticket();
    assert!(s.heap.acquire_pin::<P>(ticket, &mut s.account).is_err());
    drop(tiles);
    let layouts = members.detach();
    crate::loan::fault::watch_source_release(ticket.identity.receipt, watched);
    if !abandon {
        cancel_checked(&mut pending, &mut s.heap, &mut s.account)?;
    }
    let groups = s.heap.loans.groups.len();
    let cursor = if abandon {
        Some(scan_prefix(&mut s.heap, &mut s.account, groups, None)?)
    } else {
        None
    };
    drop(pending);
    assert_eq!(crate::loan::fault::source_at_witness(), Some(0));
    assert!(source_pins.iter().all(|pin| pin.strong_count() == 0));
    drop(source_pins);
    let cursor = if let Some(cursor) = cursor {
        Some(group_witness(&mut s.heap, &mut s.account, ticket, cursor)?)
    } else {
        None
    };
    reconcile(&mut s.heap, &mut s.account, ticket, cursor)?;
    if abandon {
        assert!(matches!(
            s.heap.take_producer_outcome(ticket, &mut s.account)?,
            crate::ProducerOutcome::Absent(_)
        ));
    }
    s.heap.retire_mask(selected, &mut s.account.ledger)?;
    super::fixture::retire_layout(&mut s.heap, &mut s.account, selected.layout())?;
    for layout in layouts {
        super::fixture::retire_layout(&mut s.heap, &mut s.account, layout)?;
    }
    super::fixture::empty(&mut s.heap, &mut s.account)
}

pub(super) fn work<P: Scalar + 'static>() -> TestResult {
    zero_route::<P>()?;
    crate::loan::lc_origin::pending::zero_work()?;
    for nonempty in [false, true] {
        for cells in [2_u64, 12] {
            for short in [false, true] {
                let counts = if cells == 2 { [1, 1, 2] } else { [2, 3, 2] };
                let mut s = setup::<P>(Limits::default(), counts)?;
                let output = LcOmegaOutput::declare(
                    &s.inventory,
                    request(&s.registry, &s.radian),
                    &mut s.account,
                )?;
                let members = super::super::inventory::readers::publish(
                    &s.inventory,
                    &s.axes,
                    &mut s.heap,
                    &mut s.account,
                    0,
                    0,
                )?;
                let tiles = members.references();
                let prepared = output
                    .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                    .map_err(|f| f.into_parts().1)?;
                let requested: &[usize] = if nonempty { &[0] } else { &[] };
                let selected = demand(&mut s.heap, &s.axes, requested, &mut s.account)?;
                // Independent fixture inventory: 30 descriptors, 16 layouts, 11 masks, 84 debit slots.
                assert_eq!(s.heap.descriptors.slots.scan_work(), 30);
                assert_eq!(s.heap.masks.layouts.scan_work(), 16);
                assert_eq!(s.heap.masks.masks.scan_work(), 11);
                assert_eq!(s.heap.loans.debits.len(), 84);
                let planning = 35 + 30 + 16 + 11 + 84 + 16 + 11 + 2 * 84;
                let payload = if cells == 0 { 0 } else { 22 };
                let words = u64::from(cells != 0);
                let exact =
                    planning + 4 * cells + 7 * words + 445 + 17 + 2 + 56 + 61 + 10 + payload;
                let remaining = exact - u64::from(short);
                let consume = s.account.limits().work - s.account.usage().work - remaining;
                s.account.ledger.work(consume, crate::loan::AT)?;
                let before = s.account.usage().work;
                let result = prepared.admit(&mut s.heap, selected, &mut s.account);
                if short {
                    let (prepared, error) = result
                        .expect_err("one below complete admission")
                        .into_parts();
                    assert!(matches!(error, LoanError::Budget(_)));
                    assert_eq!(s.account.usage().work - before, planning);
                    drop(prepared);
                } else {
                    let mut owner = result.map_err(|f| f.into_parts().1)?;
                    assert_eq!(s.account.usage().work - before, exact);
                    assert_eq!(s.account.limits().work, s.account.usage().work);
                    owner.cancel(&mut s.heap, &mut s.account)?;
                }
            }
        }
    }
    Ok(())
}

/// Every actual allocation boundary preserves the original prepared facade on refusal.
pub(super) fn prefixes<P: Scalar + 'static>() -> TestResult {
    for small in [false, true] {
        let allocations = 29;
        for fail_after in 0..=allocations {
            let counts = if small { [1, 1, 2] } else { [2, 3, 2] };
            let mut s = setup::<P>(Limits::default(), counts)?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let members = super::super::inventory::readers::publish(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                0,
                0,
            )?;
            let tiles = members.references();
            let prepared = output
                .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
            let closure = std::rc::Rc::as_ptr(&prepared.receipt.closure);
            let correspondence = std::rc::Rc::as_ptr(&prepared.correspondence);
            let key = prepared.declaration.key;
            let before = s.heap.loans.debits.iter().flatten().count();
            let injection = crate::loan::fault::Injection::after(fail_after);
            let result = prepared.admit(&mut s.heap, selected, &mut s.account);
            let prefix = crate::loan::fault::Injection::prefix();
            drop(injection);
            let mut pending = match result {
                Ok(owner) => {
                    assert_eq!(fail_after, allocations);
                    owner
                }
                Err(failure) => {
                    assert!(fail_after < allocations);
                    let (prepared, error) = failure.into_parts();
                    let allocation_refused = matches!(
                        error,
                        LoanError::AllocationRefused | LoanError::Heap(crate::HeapError::Capacity)
                    );
                    assert!(allocation_refused, "unexpected refusal: {error:?}");
                    assert_eq!(std::rc::Rc::as_ptr(&prepared.receipt.closure), closure);
                    assert_eq!(
                        std::rc::Rc::as_ptr(&prepared.correspondence),
                        correspondence
                    );
                    assert_eq!(prepared.declaration.key, key);
                    assert_eq!(s.heap.loans.debits.iter().flatten().count(), before);
                    let retained = [0, 1, 3, 5, 7]
                        .into_iter()
                        .filter(|site| *site < fail_after)
                        .count();
                    assert_eq!(prefix, [fail_after, retained, fail_after - retained]);
                    prepared
                        .admit(&mut s.heap, selected, &mut s.account)
                        .map_err(|f| f.into_parts().1)?
                }
            };
            pending.cancel(&mut s.heap, &mut s.account)?;
            drop(pending);
            drop(tiles);
            s.heap.retire_mask(selected, &mut s.account.ledger)?;
            super::fixture::retire_layout(&mut s.heap, &mut s.account, selected.layout())?;
            members.finish(&mut s.heap, &mut s.account)?;
        }
    }
    Ok(())
}

/// A reachable legacy-descriptor history raises the native group increment above preparation's
/// earlier ceiling, allowing exact retained neighbours on the unchanged original account.
pub(super) fn bytes<P: Scalar + 'static>() -> TestResult {
    crate::loan::lc_origin::pending::zero_bytes::<P>()?;
    for small in [false, true] {
        let payload = if small {
            3 * 64 + 56 + 8
        } else {
            3 * if size_of::<P>() == 4 { 64 } else { 128 } + 56 + 48
        };
        // 53 new group rows, ten debit rows and one layout/mask row each;
        // the legacy descriptor capacity is already 120 and is reused.
        let retained = 2_432 + payload + 53 * 1_032 + 10 * 264 + 2 * 32;
        let old = 30 * 1_032 + 84 * 264 + 16 * 32 + 11 * 32;
        let scratch = 17 * 264 + 5 * 16 + 3 * 248 + 2 * 24 + 1_480 + 72 + 920;
        assert_eq!(old, 54_000);
        assert_eq!(scratch, 7_832);
        let peak = retained + 2 * old + scratch;
        for staged in [false, true] {
            for short in [false, true] {
                let required = u64::try_from(if staged { peak } else { retained })?;
                let limit = required - u64::from(short);
                let mut limits = Limits::default();
                if staged {
                    limits.staged_payload_bytes = limit;
                } else {
                    limits.output_bytes = limit;
                }
                let counts = if small { [1, 1, 2] } else { [2, 3, 2] };
                let mut s = setup::<P>(limits, counts)?;
                let output = LcOmegaOutput::declare(
                    &s.inventory,
                    request(&s.registry, &s.radian),
                    &mut s.account,
                )?;
                let members = super::super::inventory::readers::publish(
                    &s.inventory,
                    &s.axes,
                    &mut s.heap,
                    &mut s.account,
                    0,
                    0,
                )?;
                let tiles = members.references();
                let prepared = output
                    .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                    .map_err(|f| f.into_parts().1)?;
                let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
                for _ in 0..50 {
                    s.heap.import_represented::<P>(
                        crate::Extent::new(0, 1)?,
                        &nrz_bindings::Descriptor::ratio(),
                        &[],
                    )?;
                }
                assert_eq!(s.heap.descriptors.slots.scan_work(), 80);
                assert_eq!(s.heap.descriptors.slots.owned_bytes()?, 120 * 256);
                assert_eq!(s.heap.loans.groups.capacity(), 30);
                let injection = short.then(|| crate::loan::fault::Injection::after(0));
                let result = prepared.admit(&mut s.heap, selected, &mut s.account);
                if short {
                    let (prepared, error) = result
                        .expect_err("one below native byte boundary")
                        .into_parts();
                    assert!(
                        matches!(error, LoanError::Heap(crate::HeapError::ResourceBytes { staged: actual, used, limit: actual_limit }) if actual == staged && used == required && actual_limit == limit)
                    );
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    drop(prepared);
                } else {
                    let mut pending = result.map_err(|f| f.into_parts().1)?;
                    assert_eq!(s.heap.loans.groups.capacity(), 83);
                    assert_eq!(s.heap.loans.debits.capacity(), 94);
                    pending.cancel(&mut s.heap, &mut s.account)?;
                }
                drop(injection);
            }
        }
    }
    Ok(())
}

/// Fixed and scan prepayments retain only the completed prefix on the original ledger.
pub(super) fn planning<P: Scalar + 'static>() -> TestResult {
    for (remaining, spent) in [
        (8, 0),
        (149, 9),
        (152, 150),
        (155, 153),
        (350, 156),
        (370, 351),
    ] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            0,
            0,
        )?;
        let tiles = members.references();
        let prepared = output
            .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
        let closure = std::rc::Rc::as_ptr(&prepared.receipt.closure);
        let consume = s.account.limits().work - s.account.usage().work - remaining;
        s.account.ledger.work(consume, crate::loan::AT)?;
        let before = s.account.usage().work;
        let injection = crate::loan::fault::Injection::after(0);
        let (prepared, error) = prepared
            .admit(&mut s.heap, selected, &mut s.account)
            .expect_err("planning prefix")
            .into_parts();
        assert!(matches!(error, LoanError::Budget(_)));
        assert_eq!(s.account.usage().work - before, spent);
        assert_eq!(std::rc::Rc::as_ptr(&prepared.receipt.closure), closure);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Foreign-account and stale adoption refuse without allocating or replacing the captured owner.
pub(super) fn refusals<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        0,
        0,
    )?;
    let tiles = members.references();
    let prepared = output
        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
    let closure = std::rc::Rc::as_ptr(&prepared.receipt.closure);
    let mut foreign = crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
    let before = foreign.usage().work;
    let injection = crate::loan::fault::Injection::after(0);
    let (mut prepared, error) = prepared
        .admit(&mut s.heap, selected, &mut foreign)
        .expect_err("foreign account")
        .into_parts();
    assert!(matches!(error, LoanError::ForeignAccount));
    assert_eq!(foreign.usage().work, before);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    let slot = prepared.receipt.header.slot;
    prepared.receipt.header.slot = usize::MAX;
    let before = s.account.usage().work;
    let (mut prepared, error) = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .expect_err("stale adoption")
        .into_parts();
    assert!(matches!(error, LoanError::WrongLoan));
    assert_eq!(s.account.usage().work - before, 1_039);
    assert_eq!(std::rc::Rc::as_ptr(&prepared.receipt.closure), closure);
    assert_eq!(prepared.receipt.header.slot, usize::MAX);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    prepared.receipt.header.slot = slot;
    let mut pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    pending.cancel(&mut s.heap, &mut s.account)?;
    Ok(())
}

/// Unwind destroys only the acquired new prefix and then the consumed prepared facade.
pub(super) fn unwind<P: Scalar + 'static>() -> TestResult {
    for small in [false, true] {
        let allocations = 29;
        for fail_after in 0..allocations {
            let counts = if small { [1, 1, 2] } else { [2, 3, 2] };
            let mut s = setup::<P>(Limits::default(), counts)?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let members = super::super::inventory::readers::publish(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                0,
                0,
            )?;
            let tiles = members.references();
            let prepared = output
                .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let closure = std::rc::Rc::downgrade(&prepared.receipt.closure);
            let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
            let injection = crate::loan::fault::Injection::unwinding_after(fail_after);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drop(prepared.admit(&mut s.heap, selected, &mut s.account));
            }));
            assert!(result.is_err());
            let retained = [0, 1, 3, 5, 7]
                .into_iter()
                .filter(|site| *site < fail_after)
                .count();
            assert_eq!(
                crate::loan::fault::Injection::prefix(),
                [fail_after, retained, fail_after - retained]
            );
            assert_eq!(closure.strong_count(), 0);
            drop(closure);
            drop(injection);
            drop(tiles);
            s.heap.retire_mask(selected, &mut s.account.ledger)?;
            super::fixture::retire_layout(&mut s.heap, &mut s.account, selected.layout())?;
            members.finish(&mut s.heap, &mut s.account)?;
        }
    }
    Ok(())
}

/// With no Work left, dropping pending storage is free and reconciliation remains conservative.
pub(super) fn exhausted_drop<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        0,
        0,
    )?;
    let tiles = members.references();
    let prepared = output
        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
    let pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let consume = s.account.limits().work - s.account.usage().work;
    s.account.ledger.work(consume, crate::loan::AT)?;
    let before = s.heap.loans.debits.iter().flatten().count();
    let injection = crate::loan::fault::Injection::after(0);
    drop(pending);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(s.account.usage().work, s.account.limits().work);
    assert!(matches!(
        s.heap.reconcile_detached(&mut s.account, 1, None),
        crate::ScanOutcome::WorkExhausted(_)
    ));
    assert_eq!(s.heap.loans.debits.iter().flatten().count(), before);
    drop(injection);
    Ok(())
}

/// Native payloads outlive a dropped heap without any destructor callback into its registry.
fn heap_first<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        0,
        0,
    )?;
    let tiles = members.references();
    let prepared = output
        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let selected = demand(&mut s.heap, &s.axes, &[0], &mut s.account)?;
    let pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    drop(tiles);
    drop(members);
    drop(s.heap);
    assert_eq!(
        pending.cell_state(0, &mut s.account)?,
        LcOutputCellState::Pending
    );
    let before = s.account.usage().work;
    let injection = crate::loan::fault::Injection::after(0);
    drop(pending);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(s.account.usage().work, before);
    drop(injection);
    Ok(())
}

/// One state inspection is charged on the unchanged account in either owner state.
pub(super) fn state_work<P: Scalar + 'static>() -> TestResult {
    for cancelled in [false, true] {
        for short in [false, true] {
            let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let members = super::super::inventory::readers::publish(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                0,
                0,
            )?;
            let tiles = members.references();
            let prepared = output
                .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let selected = demand(&mut s.heap, &s.axes, &[0], &mut s.account)?;
            let mut pending = prepared
                .admit(&mut s.heap, selected, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            if cancelled {
                pending.cancel(&mut s.heap, &mut s.account)?;
            }
            let mut foreign =
                crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
            let before = foreign.usage().work;
            assert!(matches!(
                pending.cell_state(0, &mut foreign),
                Err(LoanError::ForeignAccount)
            ));
            assert_eq!(foreign.usage().work, before);
            let before = s.account.usage().work;
            let bounds = pending.cell_state(12, &mut s.account);
            assert!(matches!(bounds, Err(LoanError::LayoutMismatch)));
            assert_eq!(s.account.usage().work, before + 1);
            let remaining = u64::from(!short);
            let consume = s.account.limits().work - s.account.usage().work - remaining;
            s.account.ledger.work(consume, crate::loan::AT)?;
            let before = s.account.usage().work;
            let result = pending.cell_state(0, &mut s.account);
            if short {
                assert!(result.is_err());
                assert_eq!(s.account.usage().work, before);
            } else {
                let expected = if cancelled {
                    LcOutputCellState::Cancelled
                } else {
                    LcOutputCellState::Pending
                };
                assert_eq!(result?, expected);
                assert_eq!(s.account.usage().work, before + 1);
            }
            if !cancelled {
                let before = s.account.usage().work;
                pending.cancel(&mut s.heap, &mut s.account)?;
                assert_eq!(s.account.usage().work, before);
            }
        }
    }
    Ok(())
}

/// Every paused slot order retains the liveness header until all dependent weak edges expire.
fn reconcile(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    ticket: crate::LoanTicket,
    mut cursor: Option<crate::ScanCursor>,
) -> TestResult {
    loop {
        match heap.reconcile_detached(account, 1, cursor) {
            crate::ScanOutcome::Complete => break,
            crate::ScanOutcome::Pending(next) => cursor = Some(next),
            other => return Err(format!("unexpected scan {other:?}").into()),
        }
        let mut header = false;
        let mut others = false;
        for debit in heap.loans.debits.iter().flatten() {
            if debit.owner
                == crate::loan::registry::DebitOwner::LcApplication(
                    crate::loan::registry::LcDebitIdentity::of(ticket.identity),
                )
                && let crate::loan::registry::AllocationKind::LcPending {
                    header: is_header, ..
                } = debit.kind
            {
                header |= is_header;
                others |= !is_header;
            }
        }
        let group_witness = matches!(
            heap.loans.group(ticket.identity)?.state,
            crate::loan::registry::State::LcOut(_)
        );
        assert!(!(others || group_witness) || header);
    }
    Ok(())
}

/// Empty mapped rectangles have ratio meaning and cannot carry the original physical quantity.
fn zero_route<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 0, 2])?;
    let Err(error) = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        0,
        0,
    ) else {
        return Err("zero mapped physical quantity unexpectedly admitted".into());
    };
    let contract = matches!(
        error.downcast_ref::<crate::OperationError>(),
        Some(crate::OperationError::Kernel(crate::KernelError::Contract))
    );
    assert!(contract, "unexpected zero-route refusal: {error}");
    Ok(())
}

/// Foreign authority leaves the owner available for cancellation on its original account.
fn cancel_checked<P: Scalar + 'static, const N: usize>(
    pending: &mut crate::LcApplicationPending<'_, P, N>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    let mut foreign = crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
    let before = foreign.usage().work;
    assert!(matches!(
        pending.cancel(heap, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(foreign.usage().work, before);
    let before = account.usage().work;
    let mut other = crate::Heap::new(crate::HeapLimits::default())?;
    assert!(matches!(
        pending.cancel(&mut other, account),
        Err(LoanError::ForeignOwner)
    ));
    assert_eq!(account.usage().work, before);
    let receipt = pending.cancel(heap, account)?;
    assert_eq!(receipt.ticket(), pending.ticket());
    assert_eq!(account.usage().work, before);
    assert!(matches!(
        pending.cancel(heap, account),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(account.usage().work, before);
    assert_eq!(
        pending.cell_state(0, account)?,
        LcOutputCellState::Cancelled
    );
    Ok(())
}

/// Advance exactly the requested combined slots without losing the paused cursor.
fn scan_prefix(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    slots: usize,
    cursor: Option<crate::ScanCursor>,
) -> TestResult<crate::ScanCursor> {
    match heap.reconcile_detached(account, slots, cursor) {
        crate::ScanOutcome::Pending(cursor) => Ok(cursor),
        other => Err(format!("unexpected scan prefix {other:?}").into()),
    }
}

/// A still-pending group blocks the expired header before any debit-dependency walk.
fn group_witness(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    ticket: crate::LoanTicket,
    cursor: crate::ScanCursor,
) -> TestResult<crate::ScanCursor> {
    use crate::loan::registry::{AllocationKind, DebitOwner, LcDebitIdentity, State};
    let owner = DebitOwner::LcApplication(LcDebitIdentity::of(ticket.identity));
    let header = heap
        .loans
        .debits
        .iter()
        .position(|entry| {
            entry.as_ref().is_some_and(|debit| {
                debit.owner == owner
                    && matches!(debit.kind, AllocationKind::LcPending { header: true, .. })
            })
        })
        .expect("LC header debit");
    let slots = heap.loans.pins.len() + header;
    let cursor = scan_prefix(heap, account, slots, Some(cursor))?;
    assert!(matches!(
        heap.loans.group(ticket.identity)?.state,
        State::LcOut(_)
    ));
    let before = account.usage().work;
    let cursor = scan_prefix(heap, account, 1, Some(cursor))?;
    // One outer slot and one group-dependency visit; no debit walk or release.
    assert_eq!(account.usage().work - before, 2);
    assert!(heap.loans.debits[header].is_some());
    assert!(matches!(
        heap.loans.group(ticket.identity)?.state,
        State::LcOut(_)
    ));
    Ok(cursor)
}
