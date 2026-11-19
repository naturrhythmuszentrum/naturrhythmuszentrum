// SPDX-License-Identifier: AGPL-3.0-only
//! Terminal facades retain private allocation identity without depending on the heap's lifetime.
use super::{Scalar, TestResult, demand, failure, request, setup};
use crate::{LcOmegaOutput, LoanError};
use nrz_bindings::Limits;

pub(in crate::loan::lc_shape::tests::output) fn surface<P: Scalar + 'static>() -> TestResult {
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
    let mut candidate = pending.evaluate(&mut s.account).map_err(failure)?;
    assert_eq!(format!("{candidate:?}"), "LcApplicationEvaluated { .. }");
    let ticket = candidate.ticket();
    assert!(matches!(
        s.heap.loan_inventory(ticket, &mut s.account),
        Err(LoanError::WrongLoan)
    ));
    assert!(s.heap.acquire_pin::<P>(ticket, &mut s.account).is_err());
    let mut foreign = crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
    let before = foreign.usage().work;
    assert!(matches!(
        candidate.cancel(&mut s.heap, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(foreign.usage().work, before);
    candidate.cancel(&mut s.heap, &mut s.account)?;
    assert!(matches!(
        s.heap.loan_inventory(ticket, &mut s.account),
        Err(LoanError::WrongLoan)
    ));
    drop(candidate);
    s.heap.retire_mask(mask, &mut s.account.ledger)?;
    super::super::fixture::retire_layout(&mut s.heap, &mut s.account, mask.layout())?;
    members.finish(&mut s.heap, &mut s.account)
}

pub(in crate::loan::lc_shape::tests::output) fn heap_first<P: Scalar + 'static>() -> TestResult {
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
        drop(members);
        let debits_before = [s.heap.loans.debits.len(), s.heap.loans.debits.capacity()];
        let before = s.account.usage().work;
        let injection = crate::loan::fault::Injection::after(0);
        if heap_first {
            drop(s.heap);
            let candidate = pending.evaluate(&mut s.account).map_err(failure)?;
            assert_eq!(s.account.usage().work - before, 436);
            drop(candidate);
        } else {
            let candidate = pending.evaluate(&mut s.account).map_err(failure)?;
            assert_eq!(
                [s.heap.loans.debits.len(), s.heap.loans.debits.capacity()],
                debits_before
            );
            let consume = s.account.limits().work - s.account.usage().work;
            s.account.ledger.work(consume, crate::loan::AT)?;
            let debits = s.heap.loans.debits.iter().flatten().count();
            drop(candidate);
            assert!(matches!(
                s.heap.reconcile_detached(&mut s.account, 1, None),
                crate::ScanOutcome::WorkExhausted(_)
            ));
            assert_eq!(s.heap.loans.debits.iter().flatten().count(), debits);
        }
        assert_eq!(closure.strong_count(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
