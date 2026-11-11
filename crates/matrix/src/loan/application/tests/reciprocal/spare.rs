// SPDX-License-Identifier: AGPL-3.0-only
//! A new account funds its own pin and uses retained spare table capacity under fixed ceilings.

use super::{Fixture, TestResult, fixture, ready};
use crate::{ApplicationCancellation, ApplicationError, ApplicationRequest, LoanError};
use nrz_bindings::{Budget, Limits};

/// Spare-capacity B=7112 headers + 240 channels + 8 filled words + 96 outcomes = 7456.
/// No table is replaced, so the peak adds only the 5360 temporary backing inventory.
/// ´test:unit:reciprocal-spare-capacity-both-byte-ceiling-neighbours´
#[test]
fn reciprocal_spare_capacity_both_byte_ceiling_neighbours() -> TestResult {
    for (cells, retained_literal, staged_literal) in [(1, 7_392, 12_752), (3, 7_456, 12_816)] {
        for staged in [false, true] {
            for succeeds in [false, true] {
                let Fixture {
                    mut heap,
                    mut account,
                    request,
                } = fixture::<f64>(cells, 1, 3, 2, false, Limits::default())?;
                let external = heap.retain_pin(&request.input, &mut account)?;
                let source = external.ticket()?;
                let operation = request.operation.clone();
                let symbols = request.destination.symbols.clone();
                let axis = request.destination.axis;
                let demand = request.demand;
                let owner = ready(heap.request_pointwise_reciprocal(request, &mut account))?;
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
                let mut limits = Limits::default();
                let retained = crate::loan::abi::application_headers::<f64>()
                    + 3 * (64 + 8 + 8)
                    + 8
                    + cells * crate::loan::abi::outcome_width();
                assert_eq!(retained, retained_literal);
                assert_eq!(retained + 5360, staged_literal);
                let required = u64::try_from(retained + usize::from(staged) * 5360)?;
                if staged {
                    limits.staged_payload_bytes = required - u64::from(!succeeds);
                } else {
                    limits.output_bytes = required - u64::from(!succeeds);
                }
                let mut continuation = crate::ExecutionAccount::new(Budget::new(limits)?)?;
                let input = heap.acquire_pin::<f64>(source, &mut continuation)?;
                drop(external);
                let destination = crate::HostTileDeclaration::check(
                    symbols,
                    axis,
                    crate::Coverage::RepresentedInputs,
                    &mut continuation,
                )?;
                let request = crate::PointwiseRequest {
                    operation,
                    input,
                    destination,
                    demand,
                    limits: crate::ExecutionLimits::default(),
                };
                let before = heap.resource_bytes()?;
                let injection =
                    crate::loan::fault::Injection::after(if succeeds { usize::MAX } else { 0 });
                match heap.request_pointwise_reciprocal(request, &mut continuation) {
                    ApplicationRequest::ApplicationReady(owner) => {
                        assert!(succeeds);
                        assert_eq!(heap.resource_bytes()? - before, retained);
                        assert!(matches!(
                            heap.cancel_application(owner, &mut continuation),
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
                        assert!(!succeeds);
                        assert_eq!((actual, used), (staged, required));
                        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                        assert_eq!(heap.resource_bytes()?, before);
                    }
                    other @ ApplicationRequest::ApplicationRefused(..) => {
                        return Err(format!("{other:?}").into());
                    }
                }
                drop(injection);
            }
        }
    }
    Ok(())
}
