// SPDX-License-Identifier: AGPL-3.0-only
//! Permanent aggregate capacity and terminal revision headroom precede all binary acquisitions.

use super::{BinaryElementwise, Limits, TestResult, admit, candidate, fixture, ready};
use crate::{
    ApplicationCancellation, ApplicationError, BinaryApplicationRequest, Heap, HeapLimits,
    LoanError,
};

/// Two source centers plus three output roles require exactly five live scalar elements.
#[test]
fn binary_aggregate_elements_real_neighbours_and_zero_capacity() -> TestResult {
    for cells in [0, 1, 9] {
        for short in [false, true] {
            if cells == 0 && short {
                continue;
            }
            let required = 5 * cells;
            let heap = Heap::new(HeapLimits::new(4096, required - usize::from(short), 4096))?;
            let mut setup = super::fixture::with_heap(
                cells,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
                heap,
                |_, _, _| (2.0, 0.0),
            )?;
            let injection =
                crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
            match admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ) {
                BinaryApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert_eq!(setup.heap.loans.elements()?, [0, required]);
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                BinaryApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(_)),
                    _,
                ) => {
                    assert!(short);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                }
                other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
            drop(injection);
        }
    }
    Ok(())
}
/// The producer's reserved terminal successor remains available after a refused new admission.
#[test]
fn binary_terminal_revision_headroom_and_final_publication() -> TestResult {
    for publish in [false, true] {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        setup.heap.loans.revision = u64::MAX - 2;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Product,
            &mut setup.account,
        ))?;
        assert_eq!(setup.heap.loans.revision, u64::MAX - 1);
        if publish {
            let owner = candidate(owner, &mut setup.account)?;
            super::published(setup.heap.publish_application(owner, &mut setup.account))?;
        } else {
            assert!(matches!(
                setup.heap.cancel_application(owner, &mut setup.account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        assert_eq!(setup.heap.loans.revision, u64::MAX);
    }
    for revision in [u64::MAX - 1, u64::MAX] {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        setup.heap.loans.revision = revision;
        let injection = crate::loan::fault::Injection::after(0);
        assert!(matches!(
            admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account
            ),
            BinaryApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::RevisionCapacity),
                _
            )
        ));
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Literal identity and token inventories do not depend on the global concurrent counter value.
#[test]
fn binary_twenty_one_identities_and_eighteen_distinct_debits() -> TestResult {
    let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 20);
    assert_eq!(
        crate::heap::identity::allocate_block::<21>(&counter),
        Err(crate::HeapError::CounterExhausted)
    );
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Relaxed),
        u64::MAX - 20
    );
    let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 21);
    let block = crate::heap::identity::allocate_block::<21>(&counter)?;
    assert_eq!(block[20], u64::MAX - 1);
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
    let identity = owner.ticket().identity;
    assert_eq!(identity.request, identity.receipt + 1);
    assert_eq!(identity.nonce, identity.receipt + 2);
    let entries = setup.heap.loans.group(identity)?.debits.entries();
    assert_eq!(entries.len(), 18);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.ok_or("debit")?.1,
            identity.receipt + 3 + u64::try_from(index)?
        );
    }
    assert_eq!(
        owner
            .core
            .evidence
            .endpoint_tokens
            .binary()
            .ok_or("side")?
            .token
            .key,
        identity.receipt + 20
    );
    Ok(())
}

/// Descriptor and native slot ceilings are fixed before either host or unrelated occupation exists.
#[test]
fn binary_descriptor_and_native_slot_neighbours() -> TestResult {
    for table in 0..3 {
        for short in [false, true] {
            let required = if table == 0 { 5 } else { 8 };
            let heap = Heap::new(HeapLimits::new(required - usize::from(short), 4096, 4096))?;
            let mut setup = super::fixture::with_heap(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
                heap,
                |_, _, _| (1.0, 0.0),
            )?;
            match table {
                0 => {}
                1 => {
                    for _ in 0..5 {
                        setup.heap.create_cell_layout(
                            setup.request.destination.axis,
                            crate::Layout::contiguous(crate::Extent::new(1, 1)?)?,
                            &mut setup.account.ledger,
                        )?;
                    }
                }
                2 => {
                    for _ in 0..4 {
                        setup.heap.create_mask(
                            setup.request.demand.layout(),
                            &[0],
                            &mut setup.account.ledger,
                        )?;
                    }
                }
                _ => return Err("table".into()),
            }
            let injection =
                crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
            match admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ) {
                BinaryApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                BinaryApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                    _,
                ) => {
                    assert!(short);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                }
                other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
            drop(injection);
        }
    }
    Ok(())
}

/// Dense storage relocation preserves managed source identities and contributes its own live elements.
#[test]
fn binary_managed_pins_survive_legacy_relocation_with_aggregate_ceiling() -> TestResult {
    for short in [false, true] {
        let heap = Heap::new(HeapLimits::new(4096, 7 - usize::from(short), 4096))?;
        let mut setup = super::fixture::with_heap(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
            heap,
            |_, _, _| (1.0, 0.0),
        )?;
        setup.heap.import_represented(
            crate::Extent::new(2, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[7.0, 8.0],
        )?;
        match admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Product,
            &mut setup.account,
        ) {
            BinaryApplicationRequest::ApplicationReady(owner) => {
                assert!(!short);
                super::published(setup.heap.publish_application(
                    candidate(owner, &mut setup.account)?,
                    &mut setup.account,
                ))?;
            }
            BinaryApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                _,
            ) => assert!(short),
            other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
    }
    Ok(())
}
