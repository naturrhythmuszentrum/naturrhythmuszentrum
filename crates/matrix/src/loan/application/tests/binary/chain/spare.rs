// SPDX-License-Identifier: AGPL-3.0-only
//! Real cancelled preparation leaves spare capacity without retaining hidden numerical ancestors.

use super::super::super::ready;
use super::{TestResult, admit, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationRequest, Elementwise, LoanError,
};
use nrz_bindings::{Budget, Limits};

/// Explicit fixture pins retain every ancestor across setup reclamation; production adds none.
fn prepared() -> TestResult<(
    crate::Heap,
    crate::PointwiseRequest<f64>,
    Vec<crate::BundlePin<f64>>,
)> {
    let (mut heap, mut account, tile, _) = super::product(1, 1)?;
    let tickets = heap
        .loans
        .groups
        .iter()
        .flatten()
        .map(|group| crate::LoanTicket {
            identity: group.identity,
        })
        .collect::<Vec<_>>();
    let mut ancestors = Vec::new();
    for ticket in tickets {
        ancestors.push(heap.acquire_pin::<f64>(ticket, &mut account)?);
    }
    let source = heap.retain_pin(tile.pin(), &mut account)?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let operation = request.operation.clone();
    let symbols = request.destination.symbols.clone();
    let axis = request.destination.axis;
    let demand = request.demand;
    let owner = ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let destination = crate::HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    Ok((
        heap,
        crate::PointwiseRequest {
            operation,
            input: source,
            destination,
            demand,
            limits: crate::ExecutionLimits::default(),
        },
        ancestors,
    ))
}

/// Delta=O=0: B=7112+240+8+32=7392 and S=B+5360=12752 on a separately funded account.
#[test]
fn chain_spare_capacity_public_byte_neighbours() -> TestResult {
    for staged in [false, true] {
        for short in [false, true] {
            let (mut heap, old, _ancestors) = prepared()?;
            let required = if staged { 12_752 } else { 7_392 };
            let mut limits = Limits::default();
            if staged {
                limits.staged_payload_bytes = required - u64::from(short);
            } else {
                limits.output_bytes = required - u64::from(short);
            }
            let mut account = crate::ExecutionAccount::new(Budget::new(limits)?)?;
            let input = heap.acquire_pin::<f64>(old.input.ticket()?, &mut account)?;
            drop(old.input);
            let destination = crate::HostTileDeclaration::check(
                old.destination.symbols,
                old.destination.axis,
                crate::Coverage::RepresentedInputs,
                &mut account,
            )?;
            let request = crate::PointwiseRequest {
                operation: old.operation,
                input,
                destination,
                demand: old.demand,
                limits: old.limits,
            };
            let before = heap.resource_bytes()?;
            match admit(&mut heap, request, Elementwise::SquareRoot, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert_eq!(heap.resource_bytes()? - before, 7392);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(crate::HeapError::ResourceBytes {
                        staged: actual,
                        used,
                        ..
                    })),
                    _,
                ) => {
                    assert!(short);
                    assert_eq!((actual, used), (staged, required));
                    assert_eq!(heap.resource_bytes()?, before);
                }
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
        }
    }
    Ok(())
}
