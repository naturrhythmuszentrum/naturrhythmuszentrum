// SPDX-License-Identifier: AGPL-3.0-only
//! Current authority, capacity, phase and private evidence refusals precede publication.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    ApplicationRequest, LoanError,
};
use nrz_bindings::Limits;

/// All newly charged per-precision destinations include the live input and unrelated legacy owners.
/// ´test:unit:pointwise-aggregate-elements-and-occupied-slots-neighbours´
#[test]
fn pointwise_aggregate_elements_and_occupied_slots_neighbours() -> TestResult {
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
            match heap.request_pointwise_square(request, &mut account) {
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
/// A ready application cannot publish, and a candidate cannot execute its law again.
/// ´test:unit:pointwise-phase-and-private-evidence-substitution-refuse´
#[test]
fn pointwise_phase_and_private_evidence_substitution_refuse() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(2, 2, 3, 2, false, Limits::default())?;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    let ticket = owner.ticket();
    let owner = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationRejected(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    let owner = candidate(owner, &mut account)?;
    let mut owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationRejected(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    owner.core.evidence.revision = 2;
    let mut owner = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::WrongLoan),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(owner.ticket(), ticket);
    owner.core.evidence.revision = 1;
    let original = owner.core.evidence.operation.clone();
    let shape = original.inputs()[0].shape().clone();
    owner.core.evidence.operation = crate::OperationDraft::map_elementwise(
        shape.clone(),
        shape,
        crate::Elementwise::Reciprocal,
    )?
    .check(crate::ExecutionLimits::default())?;
    let mut owner = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    owner.core.evidence.operation = original;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    assert_eq!(
        tile.reader(&heap, &mut account)?.read_component(
            crate::HostChannel::Center,
            0,
            0,
            &mut account
        )?,
        4.0
    );
    assert!(matches!(
        heap.probe_loan(ticket, &mut account),
        Err(LoanError::DuplicateReturn)
    ));
    Ok(())
}
/// Detached numerical evaluation coexists with the ordinary exclusive execution lease.
/// ´test:unit:pointwise-evaluates-during-unrelated-exclusive-binding´
#[test]
fn pointwise_evaluates_during_unrelated_exclusive_binding() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    let input = heap.import_represented(
        crate::Extent::new(1, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &[17.0],
    )?;
    let bound = crate::Binding::new(
        crate::binding_testing::operation(1, 1)?,
        vec![input.value()],
        crate::ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let owner = candidate(owner, &mut account)?;
    let ordinary = bound.evaluate(bound.normalized_inputs()?)?;
    bound.publish(ordinary)?;
    published(heap.publish_application(owner, &mut account))?;
    Ok(())
}
/// The reserved terminal successor remains usable after all ordinary revision headroom is gone.
/// ´test:unit:pointwise-terminal-revision-headroom-and-identity-overflow´
#[test]
fn pointwise_terminal_revision_headroom_and_identity_overflow() -> TestResult {
    for succeeds in [false, true] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture::<f64>(0, 1, 3, 0, false, Limits::default())?;
        heap.loans.revision = u64::MAX - if succeeds { 2 } else { 1 };
        match heap.request_pointwise_square(request, &mut account) {
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
/// ´test:unit:pointwise-native-mask-and-layout-slot-neighbours´
#[test]
fn pointwise_native_mask_and_layout_slot_neighbours() -> TestResult {
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
            match heap.request_pointwise_square(request, &mut account) {
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
/// Represented-singleton rounding is checked against independently decoded exact dyadic products.
/// ´test:unit:pointwise-singleton-rounding-independent-dyadic-oracle´
#[test]
fn pointwise_singleton_rounding_independent_dyadic_oracle() -> TestResult {
    singleton::<f32>()?;
    singleton::<f64>()
}
/// One format's exact represented input has a nontrivial rounded square.
fn singleton<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let value = P::ONE.next_up();
    let mut setup = fixture::<P>(1, 1, 1, 2, false, Limits::default())?;
    super::refusal::replace_source(&mut setup, |_, _| (value, P::ZERO))?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_square(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    let nominal = crate::kernel_oracle::Rational::binary(value.to_f64())?;
    let expected = nominal.multiply(nominal)?;
    let center = reader
        .read_component(crate::HostChannel::Center, 0, 0, &mut setup.account)?
        .to_f64();
    for channel in [crate::HostChannel::Total, crate::HostChannel::Error] {
        let radius = reader
            .read_component(channel, 0, 0, &mut setup.account)?
            .to_f64();
        assert!(expected.covered(center, radius)?);
        assert!(radius > 0.0);
    }
    Ok(())
}
