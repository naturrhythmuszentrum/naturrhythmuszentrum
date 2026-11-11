// SPDX-License-Identifier: AGPL-3.0-only
//! Both source edges follow acquired-prefix cleanup and the common final-witness protocol.

use super::{
    BinaryElementwise, Limits, TestResult, admit, candidate, fixture, leave, published, ready,
};
use crate::loan::fault::{self, Injection};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    BinaryApplicationRequest, LoanError, ProducerOutcome,
};
use std::rc::Rc;

/// The same common scans reconcile abandoned producers and reclaim expired source/result owners.
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
/// Six table replacements, nine channel buffers, four shells, three extras, one side, four staging/core sites.
#[test]
fn binary_every_preparation_failure_and_unwind_closes_both_pins() -> TestResult {
    for unwind in [false, true] {
        for prefix in 0..31 {
            prefix_case(prefix, unwind)?;
        }
    }
    Ok(())
}
/// Literal acquisition sites include the two-element extent at 25 and the side box at 26.
fn prefix_case(prefix: usize, unwind: bool) -> TestResult {
    let mut setup = fixture(
        2,
        2,
        [3; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
    )?;
    let pins = [
        Rc::downgrade(&setup.request.left.record),
        Rc::downgrade(&setup.request.right.record),
    ];
    let before = setup.heap.resource_bytes()?;
    let released = fault::numerical_releases();
    let work = setup.account.usage().work;
    let injection = if unwind {
        Injection::unwinding_after(prefix)
    } else {
        Injection::after(prefix)
    };
    if unwind {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _result = admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            );
        }));
        assert!(result.is_err());
        assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [0; 2]);
    } else {
        let request = match admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Product,
            &mut setup.account,
        ) {
            BinaryApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(
                    LoanError::AllocationRefused | LoanError::Heap(crate::HeapError::Capacity),
                ),
                request,
            ) => request,
            other => return Err(format!("prefix {prefix}: {other:?}").into()),
        };
        assert_eq!(Rc::as_ptr(&request.left.record), pins[0].as_ptr());
        assert_eq!(Rc::as_ptr(&request.right.record), pins[1].as_ptr());
        drop(request);
    }
    assert_eq!(Injection::attempts(), prefix + 1);
    assert_eq!(setup.account.usage().work - work, 394);
    let retained = [1, 2, 3, 5, 7, 9]
        .into_iter()
        .filter(|boundary| prefix >= *boundary)
        .count();
    assert_eq!(Injection::prefix(), [prefix, retained, prefix - retained]);
    let deltas = [0, 3096, 5736, 5776, 5776, 6544, 6544, 6576, 6576, 6608];
    assert_eq!(setup.heap.resource_bytes()?, before + deltas[prefix.min(9)]);
    let slabs = [10, 13, 16]
        .into_iter()
        .filter(|site| *site < prefix)
        .count();
    assert_eq!(
        fault::numerical_releases(),
        [
            released[0] + slabs,
            released[1] + 4 * slabs,
            released[2] + 64 * slabs
        ]
    );
    assert_eq!(setup.heap.loans.pending, 0);
    drop(injection);
    Ok(())
}
/// Manufacture only public owning states; no completion or liveness field is forged.
fn phase(
    mut owner: crate::OwnedApplication<f64>,
    phase: usize,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<crate::OwnedApplication<f64>> {
    if phase == 1 || phase == 2 {
        owner = candidate(owner, account)?;
    }
    match phase {
        0 | 1 => Ok(owner),
        2 => {
            let injection = Injection::after(0);
            let result = match heap.publish_application(owner, account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => Ok(owner),
                other => Err(format!("{other:?}").into()),
            };
            drop(injection);
            result
        }
        3 => {
            let mut foreign =
                crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
            match owner.evaluate(&mut foreign) {
                ApplicationEvaluation::ApplicationRejected(
                    ApplicationError::Loan(LoanError::ForeignAccount),
                    owner,
                ) => Ok(owner),
                other => Err(format!("{other:?}").into()),
            }
        }
        4 => {
            leave(account, 2)?;
            match owner.evaluate(account) {
                ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => Ok(owner),
                other => Err(format!("{other:?}").into()),
            }
        }
        _ => Err("phase".into()),
    }
}
/// Ready, candidate, repair, rejected and not-started all own both prepaid cancellation edges.
#[test]
fn binary_every_detached_state_cancels_with_exhausted_work() -> TestResult {
    for state in 0..5 {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Quotient,
            Limits::default(),
        )?;
        let pins = [
            Rc::downgrade(&setup.request.left.record),
            Rc::downgrade(&setup.request.right.record),
        ];
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Quotient,
            &mut setup.account,
        ))?;
        let owner = phase(owner, state, &mut setup.heap, &mut setup.account)?;
        let witness = Rc::downgrade(&owner.core.host.witness);
        let ticket = owner.ticket();
        leave(&mut setup.account, 0)?;
        let before = setup.account.usage().work;
        assert!(
            matches!(setup.heap.cancel_application(owner, &mut setup.account), ApplicationCancellation::ApplicationCancelled(receipt) if receipt.ticket() == ticket)
        );
        assert_eq!(setup.account.usage().work, before);
        assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [0; 2]);
        assert_eq!(witness.strong_count(), 0);
        assert_eq!(setup.heap.loans.pending, 0);
    }
    Ok(())
}
/// Independent extra Left, Right and both pins do not merge either source's lifetime.
#[test]
fn binary_drop_and_unwind_release_both_before_witness_and_abandon_once() -> TestResult {
    for state in 0..4 {
        for extras in 0..4 {
            for watched in 0..2 {
                for unwind in [false, true] {
                    abandoned(state, extras, watched, unwind)?;
                }
            }
        }
    }
    Ok(())
}
/// The monitor observes each edge in separate runs at the actual final witness destructor.
fn abandoned(state: usize, extras: usize, watched: usize, unwind: bool) -> TestResult {
    let mut setup = fixture(
        1,
        1,
        [1; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
    )?;
    let pins = [
        Rc::downgrade(&setup.request.left.record),
        Rc::downgrade(&setup.request.right.record),
    ];
    let mut retained = Vec::new();
    for (index, pin) in [&setup.request.left, &setup.request.right]
        .into_iter()
        .enumerate()
    {
        if extras & (1 << index) != 0 {
            retained.push(setup.heap.retain_pin(pin, &mut setup.account)?);
        }
    }
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        BinaryElementwise::Product,
        &mut setup.account,
    ))?;
    let owner = phase(owner, state, &mut setup.heap, &mut setup.account)?;
    reconcile(&mut setup.heap, &mut setup.account);
    assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [1; 2]);
    let ticket = owner.ticket();
    fault::watch_source_release(ticket.identity.receipt, pins[watched].clone());
    if unwind {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owner = owner;
                panic!("binary detached unwind");
            }))
            .is_err()
        );
    } else {
        drop(owner);
    }
    assert_eq!(fault::source_at_witness(), Some(0));
    assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [0; 2]);
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(
        matches!(setup.heap.take_producer_outcome(ticket, &mut setup.account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
    );
    assert_eq!(
        setup
            .heap
            .take_producer_outcome(ticket, &mut setup.account)?,
        ProducerOutcome::AlreadyDelivered
    );
    for pin in &retained {
        assert!(
            pin.historical_reader(&mut setup.account)?
                .contains(0, &mut setup.account)?
        );
    }
    drop(retained);
    reconcile(&mut setup.heap, &mut setup.account);
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    Ok(())
}
/// Published evidence retains the side box while both source payloads can retire independently.
#[test]
fn binary_result_survives_either_source_both_sources_and_heap_destruction() -> TestResult {
    for keep in 0..4 {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let sources = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        let mut extras = Vec::new();
        for (index, pin) in [&setup.request.left, &setup.request.right]
            .into_iter()
            .enumerate()
        {
            if keep & (1 << index) != 0 {
                extras.push(setup.heap.retain_pin(pin, &mut setup.account)?);
            }
        }
        let demand = setup.request.demand;
        let owner = candidate(
            ready(admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ))?,
            &mut setup.account,
        )?;
        setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
        let (tile, receipt) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        reconcile(&mut setup.heap, &mut setup.account);
        for (index, ticket) in sources.into_iter().enumerate() {
            assert_eq!(
                setup.heap.loans.group(ticket.identity).is_ok(),
                keep & (1 << index) != 0
            );
        }
        assert_eq!(
            tile.reader(&setup.heap, &mut setup.account)?
                .read_component(crate::HostChannel::Center, 0, 0, &mut setup.account)?
                .to_bits(),
            (-6.0_f64).to_bits()
        );
        drop(extras);
        reconcile(&mut setup.heap, &mut setup.account);
        drop(tile);
        let pin = setup
            .heap
            .acquire_pin::<f64>(receipt.ticket(), &mut setup.account)?;
        drop(setup.heap);
        assert_eq!(
            pin.historical_reader(&mut setup.account)?
                .read_component(crate::HostChannel::Center, 0, 0, &mut setup.account)?
                .to_bits(),
            (-6.0_f64).to_bits()
        );
    }
    Ok(())
}
/// Detached numerical owners remain sufficient after the heap facade ends.
#[test]
fn binary_heap_drop_before_evaluation_and_repair_destruction() -> TestResult {
    for state in 0..3 {
        for unwind in [false, true] {
            let mut setup = fixture(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let owner = ready(admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ))?;
            let mut owner = phase(owner, state, &mut setup.heap, &mut setup.account)?;
            drop(setup.heap);
            if state == 0 {
                owner = candidate(owner, &mut setup.account)?;
            }
            let released = fault::numerical_releases();
            let before = setup.account.usage().work;
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("binary heap-drop unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(
                fault::numerical_releases(),
                [released[0] + 5, released[1] + 5, released[2] + 320]
            );
            assert_eq!(setup.account.usage().work, before);
        }
    }
    Ok(())
}

/// Exact-growth final vacant tables retain five descriptor/group rows, seven native rows and 28 debits.
#[test]
fn binary_final_vacant_residence_and_no_duplicate_side_reclaim() -> TestResult {
    let mut setup = fixture(
        1,
        1,
        [1; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
    )?;
    let demand = setup.request.demand;
    let owner = candidate(
        ready(admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Product,
            &mut setup.account,
        ))?,
        &mut setup.account,
    )?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    drop(tile);
    reconcile(&mut setup.heap, &mut setup.account);
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    assert!(setup.heap.loans.debits.iter().all(Option::is_none));
    let vacant = 48 + 5 * (256 + 1032) + 7 * 32 + 28 * 264 + 3 * 40;
    assert_eq!(vacant, 14_224);
    assert_eq!(setup.heap.resource_bytes()?, vacant);
    reconcile(&mut setup.heap, &mut setup.account);
    assert_eq!(setup.heap.resource_bytes()?, vacant);
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    Ok(())
}

/// Failed traversal prepayment retains both pins; dropping it never asks the exhausted ledger for Work.
#[test]
fn binary_not_started_drop_and_unwind_release_both_before_witness() -> TestResult {
    for watched in 0..2 {
        for unwind in [false, true] {
            let mut setup = fixture(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let pins = [
                Rc::downgrade(&setup.request.left.record),
                Rc::downgrade(&setup.request.right.record),
            ];
            let owner = ready(admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ))?;
            let owner = phase(owner, 4, &mut setup.heap, &mut setup.account)?;
            fault::watch_source_release(owner.ticket().identity.receipt, pins[watched].clone());
            let before = setup.account.usage().work;
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("binary not-started unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(fault::source_at_witness(), Some(0));
            assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [0; 2]);
            assert_eq!(setup.account.usage().work, before);
        }
    }
    Ok(())
}

/// The sole new publication shell also closes its unwind prefix with both transferred edges.
#[test]
fn binary_late_publication_unwind_drops_both_pins_and_abandons_once() -> TestResult {
    let mut setup = fixture(
        1,
        1,
        [1; 2],
        2,
        false,
        BinaryElementwise::Quotient,
        Limits::default(),
    )?;
    let pins = [
        Rc::downgrade(&setup.request.left.record),
        Rc::downgrade(&setup.request.right.record),
    ];
    let owner = candidate(
        ready(admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Quotient,
            &mut setup.account,
        ))?,
        &mut setup.account,
    )?;
    let ticket = owner.ticket();
    let injection = Injection::unwinding_after(0);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _result = setup.heap.publish_application(owner, &mut setup.account);
        }))
        .is_err()
    );
    assert_eq!(Injection::attempts(), 1);
    assert_eq!(pins.each_ref().map(std::rc::Weak::strong_count), [0; 2]);
    drop(injection);
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(
        matches!(setup.heap.take_producer_outcome(ticket, &mut setup.account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
    );
    assert_eq!(
        setup
            .heap
            .take_producer_outcome(ticket, &mut setup.account)?,
        ProducerOutcome::AlreadyDelivered
    );
    Ok(())
}

/// Registry-only results and the last historical pin each destroy their three slabs exactly once.
#[test]
fn binary_registry_only_and_last_pin_drop_and_unwind() -> TestResult {
    for historical in [false, true] {
        for unwind in [false, true] {
            let mut setup = fixture(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let owner = candidate(
                ready(admit(
                    &mut setup.heap,
                    setup.request,
                    BinaryElementwise::Product,
                    &mut setup.account,
                ))?,
                &mut setup.account,
            )?;
            let (tile, receipt) =
                published(setup.heap.publish_application(owner, &mut setup.account))?;
            reconcile(&mut setup.heap, &mut setup.account);
            drop(tile);
            if historical {
                let pin = setup
                    .heap
                    .acquire_pin::<f64>(receipt.ticket(), &mut setup.account)?;
                drop(setup.heap);
                let released = fault::numerical_releases();
                let before = setup.account.usage().work;
                if unwind {
                    assert!(
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let _pin = pin;
                            panic!("binary last-pin unwind");
                        }))
                        .is_err()
                    );
                } else {
                    drop(pin);
                }
                assert_eq!(
                    fault::numerical_releases(),
                    [released[0] + 3, released[1] + 3, released[2] + 192]
                );
                assert_eq!(setup.account.usage().work, before);
            } else {
                let released = fault::numerical_releases();
                let before = setup.account.usage().work;
                if unwind {
                    assert!(
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let _heap = setup.heap;
                            panic!("binary registry-only unwind");
                        }))
                        .is_err()
                    );
                } else {
                    drop(setup.heap);
                }
                assert_eq!(
                    fault::numerical_releases(),
                    [released[0] + 3, released[1] + 3, released[2] + 192]
                );
                assert_eq!(setup.account.usage().work, before);
            }
        }
    }
    Ok(())
}
