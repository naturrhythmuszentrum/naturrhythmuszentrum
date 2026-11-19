// SPDX-License-Identifier: AGPL-3.0-only
//! Public sealed results are compared with the independent captured-source oracle.
mod lifetime;
mod resources;
use super::{TestResult, demand, request, setup};
use crate::{
    ExecutionAccount, Heap, HostChannel, LcApplicationEvaluated, LcOmegaOutput, LcOutcomeKind,
    LoanError,
};
pub(super) use lifetime::{history, stale};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;
pub(super) use resources::{reader_work, uses};
/// Final component oracle from the independent public captured-source path.
type ExpectedCell<P> = (usize, [u16; 3], Option<[P; 3]>);

/// Compare every public field and both reader modes without allocation.
pub(super) fn images<P: Scalar + 'static>(
    candidate: LcApplicationEvaluated<'_, P, 5>,
    expected: &[ExpectedCell<P>],
    admitted_demand: crate::MaskVersion,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let ticket = candidate.ticket();
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    let before = account.usage().work;
    let result = candidate.publish(heap, account).map_err(|f| f.error)?;
    assert_eq!(account.usage().work - before, 639);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(result.ticket(), ticket);
    crate::loan::lc_origin::pending::publication::checks::published(&result, heap);
    for current in [false, true] {
        let before = account.usage().work;
        let reader = if current {
            result.current_reader(heap, account)?
        } else {
            result.historical_reader(account)?
        };
        assert_eq!(
            account.usage().work - before,
            if current { 161 } else { 155 }
        );
        assert_eq!(reader.demand(), admitted_demand);
        let assurance = reader.assurance();
        assert_eq!(assurance.coverage, crate::Coverage::RepresentedInputs);
        assert_eq!(
            assurance.error_scope,
            crate::NumericalErrorScope::LocalOperation
        );
        assert_eq!(assurance.certainty, crate::LcCertainty::Undecided);
        for cell in 0..12 {
            let wanted = expected.iter().find(|entry| entry.0 == cell);
            let code = wanted.map_or([0; 3], |entry| entry.1);
            let kind = match code[0] {
                0 => LcOutcomeKind::NotRequested,
                2 => LcOutcomeKind::Filled,
                3 => LcOutcomeKind::Invalid,
                4 => LcOutcomeKind::Undecided,
                5 => LcOutcomeKind::Refused,
                _ => return Err("nonterminal oracle".into()),
            };
            let before = account.usage().work;
            let outcome = reader.outcome(cell, account)?;
            assert_eq!(account.usage().work - before, 1);
            assert_eq!(
                (
                    outcome.cell,
                    outcome.kind,
                    u16::from(outcome.target),
                    outcome.reason
                ),
                (cell, kind, code[1], code[2])
            );
            for predicate in [
                LcOutcomeKind::Filled,
                LcOutcomeKind::Invalid,
                LcOutcomeKind::Undecided,
                LcOutcomeKind::Refused,
                LcOutcomeKind::NotRequested,
            ] {
                assert_eq!(
                    reader.view(predicate).contains(cell, account)?,
                    kind != LcOutcomeKind::NotRequested && kind == predicate
                );
            }
            for (index, channel) in HostChannel::ALL.into_iter().enumerate() {
                let before = account.usage().work;
                let value = reader.read(channel, cell, account);
                assert_eq!(account.usage().work - before, 6);
                if let Some((_, _, Some(values))) = wanted {
                    assert_eq!(value?.to_f64().to_bits(), values[index].to_f64().to_bits());
                } else {
                    assert!(matches!(value, Err(LoanError::CoverageMismatch)));
                }
            }
        }
    }
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    let foreign = Heap::new(crate::HeapLimits::default())?;
    let before = account.usage().work;
    assert!(matches!(
        result.current_reader(&foreign, account),
        Err(LoanError::ForeignOwner)
    ));
    assert_eq!(account.usage().work, before);
    assert!(matches!(
        heap.loan_inventory(ticket, account),
        Err(LoanError::WrongLoan)
    ));
    assert!(heap.acquire_pin::<P>(ticket, account).is_err());
    assert!(matches!(
        heap.take_producer_outcome(ticket, account)?,
        crate::ProducerOutcome::AlreadyDelivered
    ));
    result.release(account).map_err(|(_, e)| e)?;
    heap.residence(account, 100_000)?;
    Ok(())
}

/// Exhaust the original ledger at the fixed prefix and publication bulk neighbors.
pub(super) fn work<P: Scalar + 'static>() -> TestResult {
    // Publication: 51 fixed + 5*91 sources + 8*C partition + 2*w + 3*3 attachments + 26 installation.
    for (counts, selected, exact) in [([2, 3, 2], vec![0, 3, 11], 639), ([1, 1, 2], vec![0], 559)] {
        for (remaining, spent, success) in [
            (50, 0, false),
            (51, 51, false),
            (exact - 1, 51, false),
            (exact, exact, true),
        ] {
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
            let mask = demand(&mut s.heap, &s.axes, &selected, &mut s.account)?;
            let pending = prepared
                .admit(&mut s.heap, mask, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let candidate = pending
                .evaluate(&mut s.account)
                .map_err(super::execution::failure)?;
            let ticket = candidate.ticket();
            let consume = s.account.limits().work - s.account.usage().work - remaining;
            s.account.ledger.work(consume, crate::loan::AT)?;
            crate::loan::fault::reset_reads();
            let injection = crate::loan::fault::Injection::after(0);
            let before = s.account.usage().work;
            let result = candidate.publish(&mut s.heap, &mut s.account);
            assert_eq!(s.account.usage().work - before, spent);
            if success {
                let result = result.map_err(|f| f.error)?;
                assert_eq!(result.ticket(), ticket);
                result.release(&mut s.account).map_err(|(_, e)| e)?;
            } else {
                let failure = result.expect_err("publication Work refusal");
                assert!(matches!(failure.error, LoanError::Budget(_)));
                assert_eq!(failure.owner.ticket(), ticket);
                let failure = failure
                    .owner
                    .publish(&mut s.heap, &mut s.account)
                    .expect_err("retry cannot refill Work");
                assert!(matches!(failure.error, LoanError::Budget(_)));
                let mut candidate = failure.owner;
                candidate.cancel(&mut s.heap, &mut s.account)?;
            }
            assert_eq!(crate::loan::fault::reads(), 0);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}

/// Independently exercise nonterminal codes, impossible causes, domain, tracking and attachment failures.
pub(super) fn partition<P: Scalar + 'static>() -> TestResult {
    for case in 0..11 {
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
            .map_err(super::execution::failure)?;
        crate::loan::lc_origin::pending::publication::checks::partition(
            candidate,
            &mut s.heap,
            &mut s.account,
            case,
        )?;
        s.heap.retire_mask(mask, &mut s.account.ledger)?;
        super::fixture::retire_layout(&mut s.heap, &mut s.account, mask.layout())?;
        members.finish(&mut s.heap, &mut s.account)?;
    }
    Ok(())
}

/// Tracing leaves arithmetic, allocation and Work behavior identical while joining the terminal seal.
pub(super) fn receipts<P: Scalar + 'static>() -> TestResult {
    for enabled in [false, true] {
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
        let join = prepared.receipt.closure.identity;
        let closure = u64::try_from(std::rc::Rc::as_ptr(&prepared.receipt.closure) as usize)?;
        let declaration = prepared.declaration.key;
        let question = s.inventory.input(0).ok_or("original query")?.question();
        let query = u64::try_from(std::ptr::from_ref(question.time()) as usize)?;
        let correspondence = u64::try_from(std::rc::Rc::as_ptr(&prepared.correspondence) as usize)?;
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let ticket = pending.ticket();
        let application = ticket.identity.request;
        let before = s.account.usage().work;
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let injection = crate::loan::fault::Injection::after(0);
        let run = || -> TestResult<_> {
            let candidate = pending
                .evaluate(&mut s.account)
                .map_err(super::execution::failure)?;
            Ok(candidate
                .publish(&mut s.heap, &mut s.account)
                .map_err(|f| f.error)?)
        };
        let result = if enabled {
            super::pending::receipts::publication(
                [
                    application,
                    join,
                    correspondence,
                    query,
                    closure,
                    declaration,
                ],
                [mask, ticket.identity.mask],
                ticket,
                before,
                run,
            )
        } else {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), run)
        }?;
        assert_eq!(s.account.usage().work - before, 1_075);
        assert_eq!(
            [
                crate::loan::fault::reads(),
                crate::loan::fault::scalars(),
                crate::loan::fault::Injection::attempts()
            ],
            [45, 9, 0]
        );
        drop(injection);
        assert_eq!(result.ticket().identity.request, application);
    }
    Ok(())
}
