// SPDX-License-Identifier: AGPL-3.0-only
//! Independent event and allocation inventories bound borrowing and additional use issue.
use super::{Scalar, TestResult, demand, request, setup};
use crate::{HostChannel, LcOmegaOutput, LcOutcomeKind, LoanError};
use nrz_bindings::Limits;

/// Each boundary consumes the original ledger after successful publication.
pub(in crate::loan::lc_shape::tests::output) fn reader_work<P: Scalar + 'static>() -> TestResult {
    terminal_work::<P>()?;
    // modes: historical borrow, current borrow, selected load, outcome query, growing use, spare use.
    for (mode, exact, prefix) in [
        (0, 155, 1),
        (1, 161, 1),
        (2, 6, 0),
        (3, 1, 0),
        (4, 479, 469),
        (5, 480, 472),
    ] {
        for short in [0, 1] {
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
            if mode == 5 {
                spare(&result, &mut s.heap, &mut s.account)?;
            }
            let reader = result.historical_reader(&mut s.account)?;
            let consume = s.account.limits().work - s.account.usage().work - exact + short;
            s.account.ledger.work(consume, crate::loan::AT)?;
            let before = s.account.usage().work;
            let injection = crate::loan::fault::Injection::after(usize::MAX);
            let attempt = match mode {
                0 => result.historical_reader(&mut s.account).map(|_| ()),
                1 => result.current_reader(&s.heap, &mut s.account).map(|_| ()),
                2 => reader
                    .read(HostChannel::Center, 0, &mut s.account)
                    .map(|_| ()),
                3 => reader
                    .view(LcOutcomeKind::Filled)
                    .contains(0, &mut s.account)
                    .map(|_| ()),
                4 | 5 => result.retain(&mut s.heap, &mut s.account).map(drop),
                _ => return Err("unknown reader mode".into()),
            };
            assert_eq!(
                s.account.usage().work - before,
                if short == 0 { exact } else { prefix }
            );
            if short == 0 {
                attempt?;
            } else {
                assert!(matches!(attempt, Err(LoanError::Budget(_))));
            }
            assert_eq!(
                crate::loan::fault::Injection::attempts(),
                if short == 0 && mode >= 4 {
                    if mode == 4 { 2 } else { 1 }
                } else {
                    0
                }
            );
            drop(injection);
        }
    }
    Ok(())
}
/// One released use leaves existing debit capacity available; no limits or live identities are edited.
fn spare<P: Scalar + 'static>(
    result: &crate::LcPublishedResult<P>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    let extra = result.retain(heap, account)?;
    extra.release(account).map_err(|(_, error)| error)?;
    assert!(matches!(
        heap.reconcile_detached(account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    Ok(())
}
/// Staged byte neighbors and every allocation prefix preserve the original use in both growth histories.
pub(in crate::loan::lc_shape::tests::output) fn uses<P: Scalar + 'static>() -> TestResult {
    for spare_slot in [false, true] {
        for case in 0..6 {
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
            if spare_slot {
                spare(&result, &mut s.heap, &mut s.account)?;
            }
            let ticket = result.ticket();
            if case == 5 {
                crate::loan::lc_origin::pending::publication::checks::receipt(
                    &result,
                    &mut s.heap,
                    &mut s.account,
                )?;
            } else if case < 2 {
                crate::loan::lc_origin::pending::publication::checks::staged(
                    &result,
                    &mut s.heap,
                    &mut s.account,
                    spare_slot,
                    u64::try_from(case)?,
                )?;
            } else {
                let prefix = case - 2;
                let sites = if spare_slot { 1 } else { 2 };
                let injection = crate::loan::fault::Injection::after(prefix);
                let extra = result.retain(&mut s.heap, &mut s.account);
                if prefix < sites {
                    assert!(matches!(extra, Err(LoanError::AllocationRefused)));
                    let retained = usize::from(!spare_slot && prefix > 0);
                    assert_eq!(
                        crate::loan::fault::Injection::prefix(),
                        [prefix, retained, prefix - retained]
                    );
                } else {
                    drop(extra?);
                }
                drop(injection);
            }
            assert_eq!(result.ticket(), ticket);
            assert!(
                result
                    .current_reader(&s.heap, &mut s.account)?
                    .view(LcOutcomeKind::Filled)
                    .contains(0, &mut s.account)?
            );
        }
    }
    Ok(())
}

/// Local delivery permits ordinary reclamation; both cancelled wrapper queries retain their demand.
fn terminal_work<P: Scalar + 'static>() -> TestResult {
    // Two state-wrapper fixtures and three reclaim prefixes (before validation, before mutation, exact).
    for mode in 0..5 {
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
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        if mode < 2 {
            crate::loan::lc_origin::pending::execution::checks::state::boundaries(
                pending,
                &mut s.heap,
                &mut s.account,
                mode == 1,
            )?;
        } else {
            let result = pending
                .evaluate(&mut s.account)
                .map_err(super::super::execution::failure)?
                .publish(&mut s.heap, &mut s.account)
                .map_err(|f| f.error)?;
            reclaim(result, &mut s.heap, &mut s.account, mode)?;
        }
    }
    Ok(())
}

/// The public scan adds one outer slot to the 1+6+7 LC root tariff, without taking a receipt.
fn reclaim<P: Scalar + 'static>(
    result: crate::LcPublishedResult<P>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    mode: usize,
) -> TestResult {
    let ticket = result.ticket();
    let crate::ScanOutcome::Pending(cursor) =
        heap.reclaim(account, ticket.identity.root.slot, None)
    else {
        return Err("reclaim prefix".into());
    };
    result.release(account).map_err(|(_, error)| error)?;
    let (remaining, spent) = match mode {
        2 => (7, 2),
        3 => (14, 8),
        4 => (15, 15),
        _ => unreachable!(),
    };
    let consume = account.limits().work - account.usage().work - remaining;
    account.ledger.work(consume, crate::loan::AT)?;
    let before = account.usage().work;
    let outcome = heap.reclaim(account, 1, Some(cursor));
    assert_eq!(account.usage().work - before, spent);
    if mode == 4 {
        assert!(matches!(outcome, crate::ScanOutcome::Pending(_)));
        assert!(heap.loans.groups[ticket.identity.root.slot].is_none());
    } else {
        assert!(matches!(outcome, crate::ScanOutcome::WorkExhausted(_)));
        assert!(matches!(
            heap.loans.group(ticket.identity)?.state,
            crate::loan::registry::State::LcPublished(_)
        ));
    }
    Ok(())
}
