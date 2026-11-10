// SPDX-License-Identifier: AGPL-3.0-only
//! Public expression witnesses and fixed metadata inventory cannot widen the selected adapter.
use super::{Fixture, TestResult, fixture};
use crate::loan::fault::Injection;
use crate::{ApplicationError, Expm1ApplicationRequest, LoanError};
use nrz_bindings::{Context, ExponentPolicy, Registry};
use std::rc::Rc;

/// Checked expression entries retain real original evidence in their original declaration snapshot.
#[test]
fn prepared_original_expression_witness_is_unsupported() -> TestResult {
    let Fixture {
        mut account,
        request,
        ..
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let context = Context::new(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::readings::algebra::limits::Limits::default(),
        nrz_bindings::readings::algebra::syntax::Environment::default(),
    )?;
    let checked = context.check(&context.parse("entry", "1 + 1")?)?;
    let declarations = Rc::new(context.freeze()?);
    let original = request.operation.operation.inputs()[0].shape();
    let shape = crate::ShapeDraft::new(
        original.domain().clone(),
        original.codomain().clone(),
        vec![crate::BlockDraft::new(
            0..1,
            0..1,
            vec![crate::EntryDraft::Checked(Box::new(checked))],
        )],
        crate::StructureScope::WholeFamily,
    )
    .check(declarations, crate::ShapeLimits::default())?;
    assert!(shape.entries()[0].checked_expression().is_some());
    let regions =
        crate::OperationDraft::copy(vec![shape]).check(crate::ExecutionLimits::default())?;
    let original = request.law.authority()?.request();
    let injection = Injection::after(0);
    let checked = crate::PreparedExpm1Operation::check(
        &request.law,
        &original.original.target,
        &original.original.context,
        &original.original.coordinates[0].coordinate,
        [-2.0, 0.0],
        40,
        1,
        regions.inputs()[0].clone(),
        regions.outputs()[0].clone(),
        request.operation.source,
        request.operation.destination,
        request.operation.axis,
        &mut account,
    );
    assert!(matches!(
        checked,
        Err(ApplicationError::Loan(LoanError::UnsupportedContract))
    ));
    assert_eq!(Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
/// Nonwrapping admission reserves all 26 identities and leaves one terminal successor usable.
#[test]
fn prepared_identity_inventory_and_terminal_revision_boundaries() -> TestResult {
    for short in [0, 1] {
        let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 26 + short);
        let result = crate::heap::identity::allocate_block::<26>(&counter);
        if short == 0 {
            let identities = result?;
            assert_eq!(identities[0], u64::MAX - 26);
            assert_eq!(identities[25], u64::MAX - 1);
            assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
        } else {
            assert!(matches!(result, Err(crate::HeapError::CounterExhausted)));
            assert_eq!(
                counter.load(std::sync::atomic::Ordering::Relaxed),
                u64::MAX - 25
            );
        }
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(0, 1, [-2.0, 0.0], 0, false)?;
        heap.loans.revision = u64::MAX - 2 + short;
        let injection = Injection::after(usize::MAX);
        match heap.request_pointwise_expm1(request, &mut account) {
            Expm1ApplicationRequest::ApplicationReady(owner) => {
                assert_eq!(short, 0);
                assert_eq!(heap.loans.revision, u64::MAX - 1);
                assert!(matches!(
                    heap.cancel_application(owner, &mut account),
                    crate::ApplicationCancellation::ApplicationCancelled(_)
                ));
                assert_eq!(heap.loans.revision, u64::MAX);
            }
            Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::RevisionCapacity),
                _,
            ) => {
                assert_eq!(short, 1);
                assert_eq!(Injection::attempts(), 0);
            }
            other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        drop(injection);
    }
    Ok(())
}
/// The shared descriptor and native ceilings are fixed before source and retained-law preparation.
#[test]
fn prepared_descriptor_ceiling_and_shared_index_growth() -> TestResult {
    for short in [0, 1] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = super::fixture_limits(
            1,
            1,
            [-2.0, 0.0],
            2,
            false,
            crate::HeapLimits {
                descriptors: 4 - short,
                ..crate::HeapLimits::default()
            },
        )?;
        let injection = Injection::after(usize::MAX);
        match heap.request_pointwise_expm1(request, &mut account) {
            Expm1ApplicationRequest::ApplicationReady(owner) => {
                assert_eq!(short, 0);
                assert_eq!(heap.loans.groups.capacity(), 4);
                assert_eq!(heap.loans.pins.capacity(), 2);
                assert_eq!(heap.loans.debits.capacity(), 30);
                assert_eq!(heap.masks.layouts.scan_work(), 2);
                assert_eq!(heap.masks.masks.scan_work(), 3);
                drop(owner);
            }
            Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                _,
            ) => {
                assert_eq!(short, 1);
                assert_eq!(Injection::attempts(), 0);
            }
            other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        drop(injection);
    }
    Ok(())
}
