// SPDX-License-Identifier: AGPL-3.0-only
//! Byte oracles enumerate canonical and independent-account table histories from the design.

use super::{TestResult, admit, prefix, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationRequest, Elementwise, LoanError,
};
use nrz_bindings::{Budget, Limits};

/// H=7040, T=5360, rows 256/1032/32/264/40, and seventeen debits are unchanged.
/// The account transition consumes a vacancy and appends one weak row: Delta grows by 264, O by 40.
/// ´test:unit:chain-public-byte-ceiling-neighbours´
#[test]
fn chain_public_byte_ceiling_neighbours() -> TestResult {
    assert_eq!(crate::loan::abi::application_headers::<f64>(), 7040);
    assert_eq!(crate::loan::abi::outcome_width(), 32);
    assert_eq!(crate::loan::abi::group_row(), 1032);
    assert_eq!(17 * 264 + 5 * 16 + 3 * 248 + 2 * 24, 5360);
    for reciprocal in [false, true] {
        for (cells, bytes) in [(0, 13_384), (1, 13_664), (3, 13_728)] {
            for staged in [false, true] {
                for short in [false, true] {
                    boundary(cells, reciprocal, bytes, staged, short)?;
                }
            }
        }
    }
    Ok(())
}

/// Upstream setup uses its own account; the tested account's immutable limit funds every own call.
fn boundary(
    cells: usize,
    reciprocal: bool,
    retained: usize,
    staged: bool,
    short: bool,
) -> TestResult {
    let (mut heap, _setup, tile, _) = prefix(cells, reciprocal)?;
    let old = if reciprocal { 20_360 } else { 14_248 };
    let peak = retained + 2 * old + 5360;
    if cells == 1 {
        assert_eq!(peak, if reciprocal { 59_744 } else { 47_520 });
    }
    let required = if staged { peak } else { retained };
    let mut limits = Limits::default();
    if staged {
        limits.staged_payload_bytes = u64::try_from(required - usize::from(short))?;
    } else {
        limits.output_bytes = u64::try_from(required - usize::from(short))?;
    }
    let ticket = tile.pin().ticket()?;
    drop(tile);
    let mut account = crate::ExecutionAccount::new(Budget::new(limits)?)?;
    account.ledger.work(3, crate::loan::AT)?;
    let input = heap.acquire_pin::<f64>(ticket, &mut account)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, input, function, 2, &mut account)?;
    let pointer = std::rc::Rc::as_ptr(&request.input.record);
    let before = heap.resource_bytes()?;
    let injection = crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
    match admit(&mut heap, request, function, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => {
            assert!(!short);
            assert_eq!(heap.resource_bytes()? - before, retained);
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
            request,
        ) => {
            assert!(short);
            assert_eq!((actual, used), (staged, u64::try_from(required)?));
            assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(heap.resource_bytes()?, before);
        }
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    }
    drop(injection);
    Ok(())
}

/// Canonical retained rows give Delta=6080, with O=14208/20320 and no copied producer side owner.
#[test]
fn chain_canonical_retained_byte_inventory() -> TestResult {
    assert_eq!(3 * (256 + 1032) + 2 * 32 + 8 * 264 + 40, 6080);
    for reciprocal in [false, true] {
        for (cells, expected) in [(0, 13_120), (1, 13_400), (3, 13_464)] {
            let (mut heap, mut account, tile, _) = prefix(cells, reciprocal)?;
            let function = if reciprocal {
                Elementwise::Reciprocal
            } else {
                Elementwise::SquareRoot
            };
            let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
            let before = heap.resource_bytes()?;
            let owner =
                super::super::super::ready(admit(&mut heap, request, function, &mut account))?;
            assert_eq!(heap.resource_bytes()? - before, expected);
            assert_eq!(
                heap.loans.elements()?,
                [0, if reciprocal { 11 * cells } else { 8 * cells }]
            );
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
    }
    Ok(())
}

/// The design gives B=13120/13400/13464, O=14208/20320 and T=5360: S=B+2O+T.
/// Product setup peaks at 34968/35248/35312; the prior root also fits each reciprocal limit.
/// Every call uses the original immutable account, with no reconciliation or extra pin row.
#[test]
fn chain_canonical_staged_byte_ceiling_neighbours() -> TestResult {
    for (cells, retained, peaks) in [
        (0, 13_120, [46_896, 59_120]),
        (1, 13_400, [47_176, 59_400]),
        (3, 13_464, [47_240, 59_464]),
    ] {
        for reciprocal in [false, true] {
            let required = peaks[usize::from(reciprocal)];
            assert_eq!(
                required,
                retained + 2 * if reciprocal { 20_320 } else { 14_208 } + 5360
            );
            for short in [false, true] {
                canonical_staged(cells, reciprocal, required, short)?;
            }
        }
    }
    Ok(())
}

/// Fixed S and S-1 limits are installed before both hosts and every canonical prefix call.
fn canonical_staged(cells: usize, reciprocal: bool, required: u64, short: bool) -> TestResult {
    let limits = Limits {
        staged_payload_bytes: required - u64::from(short),
        ..Limits::default()
    };
    let super::super::Fixture {
        mut heap,
        mut account,
        request: original,
    } = super::super::custom(
        cells,
        1,
        [1; 2],
        2,
        false,
        crate::BinaryElementwise::Product,
        limits,
        |operand, _, _| (if operand == 0 { 2.0 } else { 3.0 }, 0.0),
    )?;
    let owner = super::super::ready(heap.request_pointwise_product(original, &mut account))?;
    let owner = super::candidate(owner, &mut account)?;
    let (mut tile, _) = super::published(heap.publish_application(owner, &mut account))?;
    if reciprocal {
        tile = super::apply(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
    }
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
    let pointer = std::rc::Rc::as_ptr(&request.input.record);
    let before = heap.resource_bytes()?;
    let injection = crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
    match admit(&mut heap, request, function, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => {
            assert!(!short);
            assert_eq!(
                heap.resource_bytes()? - before,
                usize::try_from(required - 2 * if reciprocal { 20_320 } else { 14_208 } - 5360)?
            );
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::Heap(crate::HeapError::ResourceBytes {
                staged: true,
                used,
                ..
            })),
            request,
        ) => {
            assert!(short);
            assert_eq!(used, required);
            assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(heap.resource_bytes()?, before);
        }
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    }
    drop(injection);
    Ok(())
}
