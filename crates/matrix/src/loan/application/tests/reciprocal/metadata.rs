// SPDX-License-Identifier: AGPL-3.0-only
//! Aggregate payload, table occupations and nonwrapping identities retain independent ceilings.

use super::{Fixture, TestResult, fixture};
use crate::{ApplicationCancellation, ApplicationError, ApplicationRequest, LoanError};
use nrz_bindings::Limits;

/// All newly charged per-precision destinations include the live input and unrelated legacy owners.
/// ´test:unit:reciprocal-aggregate-elements-and-occupied-slots-neighbours´
#[test]
fn reciprocal_aggregate_elements_and_occupied_slots_neighbours() -> TestResult {
    for capacity in [false, true] {
        for succeeds in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(3, 1, 3, 2, false, Limits::default())?;
            heap.import_represented(
                crate::Extent::new(2, 1)?,
                &nrz_bindings::Descriptor::ratio(),
                &[7.0, 8.0],
            )?;
            // Nine live input entries, two unrelated entries, nine complete result entries.
            heap.limits = if capacity {
                crate::HeapLimits::new(4096, 20 - usize::from(!succeeds), 4096)
            } else {
                crate::HeapLimits::new(7 - usize::from(!succeeds), 4096, 4096)
            };
            let before = heap.loans.elements()?;
            match heap.request_pointwise_reciprocal(request, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => {
                    assert!(succeeds);
                    assert_eq!(heap.loans.elements()?, [0, 18]);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                    _,
                ) => {
                    assert!(!succeeds);
                    assert_eq!(heap.loans.elements()?, before);
                }
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
        }
    }
    Ok(())
}
/// The reserved terminal successor remains usable after all ordinary revision headroom is gone.
/// ´test:unit:reciprocal-terminal-revision-headroom-and-identity-overflow´
#[test]
fn reciprocal_terminal_revision_headroom_and_identity_overflow() -> TestResult {
    for succeeds in [false, true] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture::<f64>(0, 1, 3, 0, false, Limits::default())?;
        heap.loans.revision = u64::MAX - if succeeds { 2 } else { 1 };
        match heap.request_pointwise_reciprocal(request, &mut account) {
            ApplicationRequest::ApplicationReady(owner) => {
                assert!(succeeds);
                assert_eq!(heap.loans.revision, u64::MAX - 1);
                assert!(matches!(
                    heap.cancel_application(owner, &mut account),
                    ApplicationCancellation::ApplicationCancelled(_)
                ));
                assert_eq!(heap.loans.revision, u64::MAX);
            }
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::RevisionCapacity),
                _,
            ) => assert!(!succeeds),
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
    }
    let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 19);
    assert_eq!(
        crate::heap::identity::allocate_block::<20>(&counter),
        Err(crate::HeapError::CounterExhausted)
    );
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Relaxed),
        u64::MAX - 19
    );
    Ok(())
}

/// The shared native-table ceiling includes unrelated ordinary masks and layouts independently.
/// ´test:unit:reciprocal-native-mask-and-layout-slot-neighbours´
#[test]
fn reciprocal_native_mask_and_layout_slot_neighbours() -> TestResult {
    for layouts in [false, true] {
        for succeeds in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
            let axis = request.destination.axis;
            let layout = request.demand.layout();
            let additional = if layouts { 6 } else { 5 };
            for _ in 0..additional {
                if layouts {
                    heap.create_cell_layout(
                        axis,
                        crate::Layout::contiguous(crate::Extent::new(1, 1)?)?,
                        &mut account.ledger,
                    )?;
                } else {
                    heap.create_mask(layout, &[0], &mut account.ledger)?;
                }
            }
            heap.limits = crate::HeapLimits::new(8 - usize::from(!succeeds), 4096, 4096);
            let before = heap.loans.elements()?;
            match heap.request_pointwise_reciprocal(request, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => {
                    assert!(succeeds);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                    _,
                ) => {
                    assert!(!succeeds);
                    assert_eq!(heap.loans.elements()?, before);
                }
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
        }
    }
    Ok(())
}
/// One live input element plus all three output roles require four aggregate elements.
#[test]
fn reciprocal_four_element_boundary_fixed_before_setup() -> TestResult {
    for succeeds in [false, true] {
        let mut setup = super::fixture_with_heap::<f64>(
            1,
            1,
            1,
            2,
            false,
            Limits::default(),
            crate::HeapLimits::new(4096, 4 - usize::from(!succeeds), 4096),
        )?;
        let injection = crate::loan::fault::Injection::after(if succeeds { usize::MAX } else { 0 });
        let ticket = setup.request.input.ticket()?;
        match setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationReady(owner) => {
                assert!(succeeds);
                assert_eq!(setup.heap.loans.elements()?, [0, 4]);
                assert!(matches!(
                    setup.heap.cancel_application(owner, &mut setup.account),
                    ApplicationCancellation::ApplicationCancelled(_)
                ));
            }
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                request,
            ) => {
                assert!(!succeeds);
                assert_eq!(request.input.ticket()?, ticket);
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                assert_eq!(setup.heap.loans.elements()?, [0, 1]);
            }
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        drop(injection);
    }
    Ok(())
}
