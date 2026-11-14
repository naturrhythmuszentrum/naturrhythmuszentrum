// SPDX-License-Identifier: AGPL-3.0-only
//! Existing residence is separately funded; each new caller's immutable ceilings bind its full attempt.

use super::{BinaryElementwise, Limits, TestResult, admit, fixture, ready};
use crate::{
    ApplicationCancellation, ApplicationError, BinaryApplicationRequest, ExecutionAccount,
    HostTileDeclaration, LoanError,
};
use nrz_bindings::Budget;

/// Prepopulate spare tables through a cancelled real application while retaining both source owners.
fn prepared(cells: usize) -> TestResult<(crate::Heap, crate::PointwiseBinaryRequest<f64>)> {
    let mut setup = fixture(
        cells,
        1,
        [1; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
    )?;
    let left = setup
        .heap
        .retain_pin(&setup.request.left, &mut setup.account)?;
    let right = setup
        .heap
        .retain_pin(&setup.request.right, &mut setup.account)?;
    let operation = setup.request.operation.clone();
    let symbols = setup.request.destination.symbols.clone();
    let axis = setup.request.destination.axis;
    let demand = setup.request.demand;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        BinaryElementwise::Product,
        &mut setup.account,
    ))?;
    assert!(matches!(
        setup.heap.cancel_application(owner, &mut setup.account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        setup.heap.reclaim(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let destination = HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut setup.account,
    )?;
    Ok((
        setup.heap,
        crate::PointwiseBinaryRequest {
            operation,
            left,
            right,
            destination,
            demand,
            limits: crate::ExecutionLimits::default(),
        },
    ))
}
/// No replacement backing: B=8656+240+8+32C and S=B+5624, with real N/N-1 limits.
#[test]
fn binary_spare_capacity_both_byte_ceiling_neighbours() -> TestResult {
    for (cells, retained, staged) in [(1, 8936, 14_560), (3, 9000, 14_624)] {
        for staging in [false, true] {
            for short in [false, true] {
                let (mut heap, old) = prepared(cells)?;
                let required = if staging { staged } else { retained };
                let mut limits = Limits::default();
                if staging {
                    limits.staged_payload_bytes = required - u64::from(short);
                } else {
                    limits.output_bytes = required - u64::from(short);
                }
                let mut account = ExecutionAccount::new(Budget::new(limits)?)?;
                let left = heap.acquire_pin::<f64>(old.left.ticket()?, &mut account)?;
                let right = heap.acquire_pin::<f64>(old.right.ticket()?, &mut account)?;
                drop(old.left);
                drop(old.right);
                let destination = HostTileDeclaration::check(
                    old.destination.symbols,
                    old.destination.axis,
                    crate::Coverage::RepresentedInputs,
                    &mut account,
                )?;
                let request = crate::PointwiseBinaryRequest {
                    operation: old.operation,
                    left,
                    right,
                    destination,
                    demand: old.demand,
                    limits: old.limits,
                };
                let before = heap.resource_bytes()?;
                let injection =
                    crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
                match admit(&mut heap, request, BinaryElementwise::Product, &mut account) {
                    BinaryApplicationRequest::ApplicationReady(owner) => {
                        assert!(!short);
                        assert_eq!(u64::try_from(heap.resource_bytes()? - before)?, retained);
                        assert!(matches!(
                            heap.cancel_application(owner, &mut account),
                            ApplicationCancellation::ApplicationCancelled(_)
                        ));
                    }
                    BinaryApplicationRequest::ApplicationRefused(
                        ApplicationError::Loan(LoanError::Heap(crate::HeapError::ResourceBytes {
                            staged: actual,
                            used,
                            ..
                        })),
                        _,
                    ) => {
                        assert!(short);
                        assert_eq!((actual, used), (staging, required));
                        assert_eq!(heap.resource_bytes()?, before);
                        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    }
                    other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                        return Err(format!("{other:?}").into());
                    }
                }
                drop(injection);
            }
        }
    }
    Ok(())
}

/// Publication's 64-byte mask shell is isolated because full public setup dominates that ceiling.
#[test]
fn binary_publication_shell_independent_byte_neighbours() -> TestResult {
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
    for staging in [false, true] {
        for short in [false, true] {
            let mut limits = Limits::default();
            if staging {
                limits.staged_payload_bytes = 64 - u64::from(short);
            } else {
                limits.output_bytes = 64 - u64::from(short);
            }
            let mut budget = Budget::new(limits)?;
            budget.work(3, crate::loan::AT)?;
            let result = crate::loan::application::publication::shell_preflight(
                &setup.heap,
                &owner.core,
                &mut budget,
            );
            assert_eq!(result.is_ok(), !short);
            if let Err(LoanError::Heap(crate::HeapError::ResourceBytes { staged, used, .. })) =
                result
            {
                assert_eq!((staged, used), (staging, 64));
            }
        }
    }
    Ok(())
}
