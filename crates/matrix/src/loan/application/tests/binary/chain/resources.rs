// SPDX-License-Identifier: AGPL-3.0-only
//! Permanent heap, invocation and identity ceilings retain the existing common admission semantics.

use super::super as binary;
use super::super::super::ready;
use super::{TestResult, admit, candidate, published, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationRequest, Elementwise, LoanError,
};
use nrz_bindings::{Budget, Limits};

/// Build the canonical prefix under permanent heap ceilings, optionally including unrelated residence.
fn fixture(
    reciprocal: bool,
    limits: crate::HeapLimits,
    legacy: bool,
) -> TestResult<(
    crate::Heap,
    crate::ExecutionAccount,
    crate::PublishedTile<f64>,
)> {
    let mut heap = crate::Heap::new(limits)?;
    if legacy {
        heap.import_represented(
            crate::Extent::new(2, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[7.0, 8.0],
        )?;
    }
    let binary::Fixture {
        mut heap,
        mut account,
        request,
    } = binary::fixture::with_heap::<f64>(
        1,
        1,
        [1; 2],
        2,
        false,
        crate::BinaryElementwise::Product,
        Limits::default(),
        heap,
        |operand, _, _| (if operand == 0 { 2.0 } else { 3.0 }, 0.0),
    )?;
    let owner = binary::ready(heap.request_pointwise_product(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (mut tile, _) = published(heap.publish_application(owner, &mut account))?;
    if reciprocal {
        tile = super::apply(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
    }
    Ok((heap, account, tile))
}

/// With all ancestors current, exact aggregate elements are 8L/11L plus separately resident legacy data.
#[test]
fn chain_fixed_element_and_descriptor_neighbours() -> TestResult {
    for reciprocal in [false, true] {
        for legacy in [false, true] {
            for descriptors in [false, true] {
                for short in [false, true] {
                    let managed = if reciprocal { 11 } else { 8 };
                    let slots = managed + usize::from(legacy);
                    let elements = managed + 2 * usize::from(legacy);
                    let limits = crate::HeapLimits::new(
                        if descriptors {
                            slots - usize::from(short)
                        } else {
                            4096
                        },
                        if descriptors {
                            4096
                        } else {
                            elements - usize::from(short)
                        },
                        4096,
                    );
                    let (mut heap, mut account, tile) = fixture(reciprocal, limits, legacy)?;
                    let function = if reciprocal {
                        Elementwise::Reciprocal
                    } else {
                        Elementwise::SquareRoot
                    };
                    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
                    capacity(&mut heap, &mut account, request, function, short)?;
                }
            }
        }
    }
    Ok(())
}

/// Resource refusal occurs at the consumer admission and attempts no private allocation.
fn capacity(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    request: crate::PointwiseRequest<f64>,
    function: Elementwise,
    short: bool,
) -> TestResult {
    let ticket = request.input.ticket()?;
    let injection = crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
    match admit(heap, request, function, account) {
        ApplicationRequest::ApplicationReady(owner) => {
            assert!(!short);
            assert!(matches!(
                heap.cancel_application(owner, account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
            request,
        ) => {
            assert!(short);
            assert_eq!(request.input.ticket()?, ticket);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        }
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    }
    drop(injection);
    Ok(())
}

/// Independent native slots reach sixteen while descriptor capacity remains otherwise sufficient.
#[test]
fn chain_fixed_native_table_neighbours() -> TestResult {
    for reciprocal in [false, true] {
        for layouts in [false, true] {
            for short in [false, true] {
                let (mut heap, mut account, tile) = fixture(
                    reciprocal,
                    crate::HeapLimits::new(16 - usize::from(short), 4096, 4096),
                    false,
                )?;
                let function = if reciprocal {
                    Elementwise::Reciprocal
                } else {
                    Elementwise::SquareRoot
                };
                let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
                let before = if layouts {
                    if reciprocal { 4 } else { 3 }
                } else if reciprocal {
                    7
                } else {
                    5
                };
                for _ in before..15 {
                    if layouts {
                        heap.create_cell_layout(
                            request.destination.axis,
                            crate::Layout::contiguous(crate::Extent::new(1, 1)?)?,
                            &mut account.ledger,
                        )?;
                    } else {
                        heap.create_mask(request.demand.layout(), &[0], &mut account.ledger)?;
                    }
                }
                capacity(&mut heap, &mut account, request, function, short)?;
            }
        }
    }
    Ok(())
}

/// Every unary invocation keeps check(1,max(12k,3L)), independently at zero and nonzero capacity.
#[test]
fn chain_invocation_work_and_scratch_neighbours() -> TestResult {
    for (cells, components, required) in [(0, 3, 36), (1, 3, 36), (65, 1, 195)] {
        for ceiling in 0..3 {
            for short in [false, true] {
                let (mut heap, mut account, tile, _) = super::product(cells, components)?;
                let mut request = request(
                    &mut heap,
                    tile.into_pin(),
                    Elementwise::SquareRoot,
                    0,
                    &mut account,
                )?;
                request.limits = crate::ExecutionLimits::new(
                    usize::from(ceiling != 0 || !short),
                    required - usize::from(ceiling == 1 && short),
                    required - usize::from(ceiling == 2 && short),
                );
                let injection =
                    crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
                match admit(&mut heap, request, Elementwise::SquareRoot, &mut account) {
                    ApplicationRequest::ApplicationReady(owner) => {
                        assert!(!short);
                        assert!(matches!(
                            heap.cancel_application(owner, &mut account),
                            ApplicationCancellation::ApplicationCancelled(_)
                        ));
                    }
                    ApplicationRequest::ApplicationRefused(
                        ApplicationError::Operation(crate::OperationError::ResourceLimit),
                        _,
                    ) => {
                        assert!(short);
                        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
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

/// Twenty identities and one reserved terminal revision are the unchanged unary entitlements.
#[test]
fn chain_identity_and_terminal_revision_boundaries() -> TestResult {
    for short in [false, true] {
        let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 20 + u64::from(short));
        assert_eq!(
            crate::heap::identity::allocate_block::<20>(&counter).is_ok(),
            !short
        );
        let (mut heap, mut account, tile, _) = super::product(0, 1)?;
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            0,
            &mut account,
        )?;
        heap.loans.revision = u64::MAX - 2 + u64::from(short);
        match admit(&mut heap, request, Elementwise::SquareRoot, &mut account) {
            ApplicationRequest::ApplicationReady(owner) => {
                assert!(!short);
                assert!(matches!(
                    heap.cancel_application(owner, &mut account),
                    ApplicationCancellation::ApplicationCancelled(_)
                ));
                assert_eq!(heap.loans.revision, u64::MAX);
            }
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::RevisionCapacity),
                _,
            ) => assert!(short),
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
    }
    Ok(())
}

/// The same complete common publication permit admits exactly a 64-byte new mask shell.
#[test]
fn chain_isolated_publication_shell_byte_neighbours() -> TestResult {
    let (mut heap, mut account, tile, _) = super::product(1, 1)?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let owner = ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    for staged in [false, true] {
        for short in [false, true] {
            let mut limits = Limits::default();
            if staged {
                limits.staged_payload_bytes = 64 - u64::from(short);
            } else {
                limits.output_bytes = 64 - u64::from(short);
            }
            let mut budget = Budget::new(limits)?;
            budget.work(3, crate::loan::AT)?;
            let result = crate::loan::application::publication::shell_preflight(
                &heap,
                &owner.core,
                &mut budget,
            );
            assert_eq!(result.is_ok(), !short);
            if let Err(LoanError::Heap(crate::HeapError::ResourceBytes {
                staged: actual,
                used,
                ..
            })) = result
            {
                assert_eq!((actual, used), (staged, 64));
            }
        }
    }
    Ok(())
}
