// SPDX-License-Identifier: AGPL-3.0-only
//! Private fixtures exercise retention without claiming that host bytes are expm1 results.

use super::super::{Attachment, Binding, RetainedExpm1};
use super::{DESTROYED, TestResult, fixture};
use crate::{
    BundlePin, Coverage, ExecutionAccount, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanError, LoanRequest, OwnedLoan, ProducerOutcome, PublicationReceipt,
    PublishedTile, Residence, ScanOutcome,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_scalar::ArithmeticContext;

/// A host fixture has one ordinary initialized scalar and no prepared completion evidence.
fn host(heap: &mut Heap, account: &mut ExecutionAccount) -> TestResult<OwnedLoan<f64>> {
    let mut symbolic = Budget::new(Limits::default())?;
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [false, false], &mut symbolic)?;
    let axis = heap.create_cell_axis(1, &mut symbolic)?;
    let declaration =
        HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, account)?;
    let mut loan = match heap.request_loan(declaration, account) {
        LoanRequest::Granted(loan) => loan,
        LoanRequest::Refused(error) => return Err(error.into()),
    };
    loan.write_cell(HostChannel::Center, 0, &[7.0], account)?;
    Ok(loan)
}
/// Capture an exact use before moving it into the detached fixture core.
fn attach(
    heap: &Heap,
    loan: &mut OwnedLoan<f64>,
    law: RetainedExpm1,
    account: &mut ExecutionAccount,
) -> TestResult {
    let binding = Binding::of(&law)?;
    loan.core.retained_law = Some(
        Attachment::prepare(law, binding, heap, &mut account.ledger).map_err(|(error, _)| error)?,
    );
    Ok(())
}
/// Publish only the ordinary host values; the private law edge is ownership evidence alone.
fn publish(
    heap: &mut Heap,
    loan: OwnedLoan<f64>,
    account: &mut ExecutionAccount,
) -> TestResult<(PublishedTile<f64>, PublicationReceipt)> {
    match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, receipt) => Ok((tile, receipt)),
        other => Err(format!("{other:?}").into()),
    }
}
/// Borrow the exact attached use through a retaining pin without acquiring any law edge.
fn law(pin: &BundlePin<f64>) -> TestResult<&RetainedExpm1> {
    Ok(&pin
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .and_then(|bundle| bundle.retained_law.as_ref())
        .ok_or("retained law attachment")?
        .law)
}
/// Both the remaining result pin and a registry-only root retain the exact graph and authority.
#[test]
fn retained_law_bundle_pin_root_and_heap_drop() -> TestResult {
    for heap_first in [false, true] {
        let destroyed = DESTROYED.get();
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let external = fixture::prepare(&mut heap, &mut account)?;
        let moved = heap.retain_retained_expm1(&external, &mut account)?;
        let expected = Binding::of(&moved)?;
        let authority = std::ptr::from_ref(external.authority()?);
        let mut loan = host(&mut heap, &mut account)?;
        attach(&heap, &mut loan, moved, &mut account)?;
        let (tile, receipt) = publish(&mut heap, loan, &mut account)?;
        assert_eq!(
            heap.loan_inventory(receipt.ticket(), &mut account)?
                .retained_law,
            Some(expected.identity.owner)
        );
        let pin = heap.retain_pin(tile.pin(), &mut account)?;
        drop((external, tile));
        assert_eq!(DESTROYED.get(), destroyed);
        assert_eq!(Binding::of(law(&pin)?)?, expected);
        assert_eq!(std::ptr::from_ref(law(&pin)?.authority()?), authority);
        assert_eq!(
            pin.historical_reader(&mut account)?.read_component(
                HostChannel::Center,
                0,
                0,
                &mut account
            )?,
            7.0
        );
        law(&pin)?.evaluate(fixture::point(-0.5), &ArithmeticContext::new()?)?;
        if heap_first {
            drop(heap);
            assert_eq!(DESTROYED.get(), destroyed);
            law(&pin)?.evaluate(fixture::point(0.0), &ArithmeticContext::new()?)?;
            drop(pin);
        } else {
            drop(pin);
            assert_eq!(DESTROYED.get(), destroyed);
            let again = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
            assert_eq!(Binding::of(law(&again)?)?, expected);
            assert!(matches!(
                heap.take_producer_outcome(receipt.ticket(), &mut account)?,
                ProducerOutcome::AlreadyDelivered
            ));
            drop(again);
            assert!(matches!(
                heap.residence(&mut account, 1000)?,
                Residence::Pending { .. }
            ));
            assert_eq!(DESTROYED.get(), destroyed + 1);
            assert!(matches!(
                heap.residence(&mut account, 1000)?,
                Residence::Empty { .. }
            ));
            drop(heap);
        }
        assert_eq!(DESTROYED.get(), destroyed + 1);
    }
    Ok(())
}
/// Two bundles and an independent use distinguish releasing one edge from destroying authority.
#[test]
fn retained_law_multiple_bundle_edges() -> TestResult {
    let destroyed = DESTROYED.get();
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let external = fixture::prepare(&mut heap, &mut account)?;
    let mut publications = Vec::new();
    for _ in 0..2 {
        let moved = heap.retain_retained_expm1(&external, &mut account)?;
        let mut loan = host(&mut heap, &mut account)?;
        attach(&heap, &mut loan, moved, &mut account)?;
        publications.push(publish(&mut heap, loan, &mut account)?);
    }
    assert_eq!(
        heap.retained_expm1_inventory(&external, &mut account)?.uses,
        3
    );
    for (tile, receipt) in publications {
        drop(tile);
        heap.take_producer_outcome(receipt.ticket(), &mut account)?;
        assert!(matches!(
            heap.reconcile_detached(&mut account, 1000, None),
            ScanOutcome::Complete
        ));
        assert!(matches!(
            heap.reclaim(&mut account, 1000, None),
            ScanOutcome::Complete
        ));
        assert_eq!(DESTROYED.get(), destroyed);
    }
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert_eq!(
        heap.retained_expm1_inventory(&external, &mut account)?.uses,
        1
    );
    drop(external);
    assert_eq!(DESTROYED.get(), destroyed + 1);
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        Residence::Empty { .. }
    ));
    Ok(())
}
/// Detached and repair owners retain their exact use across heap drop and lexical unwinding.
#[test]
fn retained_law_detached_repair_and_unwind() -> TestResult {
    for repair in [false, true] {
        for unwind in [false, true] {
            let destroyed = DESTROYED.get();
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut account = fixture::account()?;
            let moved = fixture::prepare(&mut heap, &mut account)?;
            let binding = Binding::of(&moved)?;
            let mut loan = host(&mut heap, &mut account)?;
            attach(&heap, &mut loan, moved, &mut account)?;
            if repair {
                let injection = crate::loan::fault::Injection::after(0);
                loan = match heap.handback(loan, &mut account) {
                    HandbackOutcome::Repair(LoanError::AllocationRefused, loan) => loan,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            let attachment = loan
                .core
                .retained_law
                .as_ref()
                .ok_or("detached attachment")?;
            assert_eq!(Binding::of(&attachment.law)?, binding);
            attachment
                .law
                .evaluate(fixture::point(-1.0), &ArithmeticContext::new()?)?;
            drop(heap);
            assert_eq!(DESTROYED.get(), destroyed);
            if unwind {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _owner = loan;
                    panic!("retained law lexical unwind fixture");
                }));
                assert!(result.is_err());
            } else {
                drop(loan);
            }
            assert_eq!(DESTROYED.get(), destroyed + 1);
        }
    }
    Ok(())
}
/// Equal statement keys and distinct represented domains cannot replace the captured exact use.
#[test]
fn retained_law_attachment_substitution_refuses() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let original = fixture::prepare(&mut heap, &mut account)?;
    let binding = Binding::of(&original)?;
    let other = fixture::prepare(&mut heap, &mut account)?;
    assert_eq!(
        original.authority()?.statement_key(),
        other.authority()?.statement_key()
    );
    let other_id = Binding::of(&other)?;
    let (error, other) = Attachment::prepare(other, binding, &heap, &mut account.ledger)
        .expect_err("equal statement has another runtime owner");
    assert!(matches!(error, LoanError::WrongLoan));
    assert_eq!(Binding::of(&other)?, other_id);
    let same_owner = heap.retain_retained_expm1(&original, &mut account)?;
    assert!(Attachment::prepare(same_owner, binding, &heap, &mut account.ledger).is_err());
    for domain in [[-1.0, 0.0], [-0.75, -0.5]] {
        let (cas, request) = fixture::request()?;
        let law = heap
            .prepare_retained_expm1(
                cas,
                &request,
                domain,
                nrz_bindings::readings::algebra::enclosure::prepared_expm1::Policy::default(),
                &ArithmeticContext::new()?,
                &mut nrz_bindings::readings::algebra::enclosure::prepared_expm1::Counts::default(),
                &mut account,
            )
            .map_err(|(error, _)| error)?;
        assert_eq!(
            original.authority()?.statement_key(),
            law.authority()?.statement_key()
        );
        assert!(Attachment::prepare(law, binding, &heap, &mut account.ledger).is_err());
    }
    Ok(())
}

/// Valid checked bound witnesses can differ behind equal program statement keys.
#[test]
fn retained_law_equal_key_distinct_bound_witness() -> TestResult {
    use nrz_bindings::readings::algebra::{
        Rational,
        enclosure::prepared_expm1::{Counts, Policy},
    };
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let first = fixture::prepare(&mut heap, &mut account)?;
    let mut proof = first.authority()?.proof().clone();
    let mut budget = Budget::new(fixture::limits())?;
    proof.endpoint_ratio = Rational::from_ratio(2, 1, &mut budget)?.pow(-40, &mut budget)?;
    assert_ne!(
        proof.endpoint_ratio,
        first.authority()?.proof().endpoint_ratio
    );
    let (mut cas, request) = fixture::request()?;
    cas.check_expm1(
        &request,
        Policy::default(),
        &proof,
        &mut budget,
        &mut Counts::default(),
    )?;
    let mut counts = Counts::default();
    let second = heap
        .prepare_retained_expm1(
            cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &ArithmeticContext::new()?,
            &mut counts,
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    assert_eq!(counts.proposals, 0);
    assert_eq!(
        first.authority()?.statement_key(),
        second.authority()?.statement_key()
    );
    assert_ne!(
        first.authority()?.proof().endpoint_ratio,
        second.authority()?.proof().endpoint_ratio
    );
    let expected = Binding::of(&first)?;
    assert!(Attachment::prepare(second, expected, &heap, &mut account.ledger).is_err());
    Ok(())
}

/// The actual payload destructor runs before the detached liveness witness expires.
#[test]
fn retained_law_destruction_precedes_witness() -> TestResult {
    for cancel in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let moved = fixture::prepare(&mut heap, &mut account)?;
        let mut loan = host(&mut heap, &mut account)?;
        attach(&heap, &mut loan, moved, &mut account)?;
        super::WATCH_WITNESS.set(Some((loan.core.cleanup.identity, DESTROYED.get())));
        super::AT_WITNESS.set(None);
        let work = account.usage().work;
        if cancel {
            assert!(matches!(
                heap.cancel_loan(loan, &mut account),
                crate::CancelOutcome::Cancelled(_)
            ));
        } else {
            drop(loan);
        }
        assert_eq!(super::AT_WITNESS.get(), Some(1));
        assert_eq!(account.usage().work, work);
        assert!(matches!(
            heap.reconcile_detached(&mut account, 1000, None),
            ScanOutcome::Complete
        ));
    }
    Ok(())
}

/// Current readers repeat the performed law validation; no unmetered pin edge is acquired.
#[test]
fn retained_law_current_attachment_repeated_work() -> TestResult {
    for short in [0, 1] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut original = fixture::account()?;
        let moved = fixture::prepare(&mut heap, &mut original)?;
        let mut loan = host(&mut heap, &mut original)?;
        attach(&heap, &mut loan, moved, &mut original)?;
        let (tile, receipt) = publish(&mut heap, loan, &mut original)?;
        assert_eq!([heap.loans.debits.len(), heap.loans.pins.len()], [11, 1]);
        // Ticket authentication 2 + current owner 15; pin preparation visits 1+11+24
        // slots and prepays 9 acquisitions/releases. Each current reader is 3+15+3.
        let mut budget = Budget::new(Limits {
            work: 7 + 62 + 2 * 21 - short,
            ..fixture::limits()
        })?;
        budget.work(7, crate::loan::AT)?;
        let mut account = ExecutionAccount::new(budget)?;
        let pin = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
        assert_eq!(account.usage().work, 69);
        let borrowed = PublishedTile { pin };
        borrowed.reader(&heap, &mut account)?;
        assert_eq!(account.usage().work, 90);
        let result = borrowed.reader(&heap, &mut account);
        if short == 0 {
            result?;
            assert_eq!(account.usage().work, 111);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
        }
        drop((tile, borrowed));
    }
    Ok(())
}

/// Every publication allocation prefix keeps the exact law on refusal and closes it on unwind.
#[test]
fn retained_law_publication_prefix_cleanup() -> TestResult {
    for unwind in [false, true] {
        for prefix in 0..3 {
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut account = fixture::account()?;
            let moved = fixture::prepare(&mut heap, &mut account)?;
            let binding = Binding::of(&moved)?;
            let mut loan = host(&mut heap, &mut account)?;
            attach(&heap, &mut loan, moved, &mut account)?;
            let ticket = loan.ticket();
            let destroyed = DESTROYED.get();
            let injection = if unwind {
                crate::loan::fault::Injection::unwinding_after(prefix)
            } else {
                crate::loan::fault::Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drop(heap.handback(loan, &mut account));
                    }))
                    .is_err()
                );
            } else {
                let repair = match heap.handback(loan, &mut account) {
                    HandbackOutcome::Repair(LoanError::AllocationRefused, repair) => repair,
                    other => return Err(format!("{other:?}").into()),
                };
                assert_eq!(DESTROYED.get(), destroyed);
                let attachment = repair.core.retained_law.as_ref().ok_or("law")?;
                assert_eq!(Binding::of(&attachment.law)?, binding);
                attachment
                    .law
                    .evaluate(fixture::point(-0.5), &ArithmeticContext::new()?)?;
                drop(repair);
            }
            let retained = usize::from(prefix != 0);
            assert_eq!(
                crate::loan::fault::Injection::prefix(),
                [prefix, retained, prefix - retained]
            );
            assert_eq!(DESTROYED.get(), destroyed + 1);
            drop(injection);
            assert!(matches!(
                heap.reconcile_detached(&mut account, 1000, None),
                ScanOutcome::Complete
            ));
            assert!(matches!(
                heap.take_producer_outcome(ticket, &mut account)?,
                ProducerOutcome::Absent(_)
            ));
            assert!(matches!(
                heap.residence(&mut account, 1000)?,
                Residence::Empty { .. }
            ));
        }
    }
    Ok(())
}

/// A registry-only root and an escaped last pin each close the actual law during unwinding.
#[test]
fn retained_law_published_root_and_pin_unwind() -> TestResult {
    for pin_last in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let moved = fixture::prepare(&mut heap, &mut account)?;
        let mut loan = host(&mut heap, &mut account)?;
        attach(&heap, &mut loan, moved, &mut account)?;
        let (tile, _) = publish(&mut heap, loan, &mut account)?;
        let destroyed = DESTROYED.get();
        let work = account.usage().work;
        if pin_last {
            let pin = tile.into_pin();
            drop(heap);
            assert_eq!(DESTROYED.get(), destroyed);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _pin = pin;
                    panic!("last historical pin unwind fixture");
                }))
                .is_err()
            );
        } else {
            drop(tile);
            assert_eq!(DESTROYED.get(), destroyed);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _heap = heap;
                    panic!("registry-only law unwind fixture");
                }))
                .is_err()
            );
        }
        assert_eq!(DESTROYED.get(), destroyed + 1);
        assert_eq!(account.usage().work, work);
    }
    Ok(())
}

#[path = "reclaim_work.rs"]
mod reclaim_work;
