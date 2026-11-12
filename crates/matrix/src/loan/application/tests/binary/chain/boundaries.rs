// SPDX-License-Identifier: AGPL-3.0-only
//! Real cumulative Work neighbours preserve original accounts and all rightful owners.

use super::super::{super::ready, leave};
use super::{TestResult, admit, candidate, prefix, request};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    ApplicationRequest, Elementwise, LoanError,
};

/// Canonical W=334/364/376 and 404/434/446; a separately acquired pin adds exactly one scan.
#[test]
fn chain_admission_work_real_neighbours() -> TestResult {
    for (cells, canonical) in [(0, [334, 404]), (1, [364, 434]), (3, [376, 446])] {
        for reciprocal in [false, true] {
            for separate in [false, true] {
                for short in [false, true] {
                    admission(
                        cells,
                        reciprocal,
                        separate,
                        canonical[usize::from(reciprocal)] + u64::from(separate),
                        short,
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Setup never replenishes an account: an independent continuation acquires its own funded pin.
fn admission(
    cells: usize,
    reciprocal: bool,
    separate: bool,
    required: u64,
    short: bool,
) -> TestResult {
    let (mut heap, mut setup, tile, _) = prefix(cells, reciprocal)?;
    let mut continuation =
        crate::ExecutionAccount::new(nrz_bindings::Budget::new(nrz_bindings::Limits::default())?)?;
    let (input, account) = if separate {
        let ticket = tile.pin().ticket()?;
        drop(tile);
        let pin = heap.acquire_pin::<f64>(ticket, &mut continuation)?;
        (pin, &mut continuation)
    } else {
        (tile.into_pin(), &mut setup)
    };
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, input, function, 2, account)?;
    let pointer = std::rc::Rc::as_ptr(&request.input.record);
    leave(account, required - u64::from(short))?;
    let before = account.usage().work;
    assert!(before > 0);
    let injection = crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
    match admit(&mut heap, request, function, account) {
        ApplicationRequest::ApplicationReady(owner) => {
            assert!(!short);
            assert_eq!(account.usage().work - before, required);
            assert!(matches!(
                heap.cancel_application(owner, account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::Budget(_)),
            request,
        ) => {
            assert!(short);
            assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        }
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    }
    drop(injection);
    Ok(())
}

/// Each literal list is the design's account visit, immediate J and ordered edge tariffs.
#[test]
fn chain_every_lineage_work_prefix_is_charged_before_allocation() -> TestResult {
    for reciprocal in [false, true] {
        let mut charges = vec![
            1,
            1,
            1,
            1,
            if reciprocal { 9 } else { 10 },
            4,
            1,
            1,
            1,
            3,
            1,
        ];
        if reciprocal {
            charges.extend([1, 1, 10, 4, 1, 1, 1, 3]);
        }
        for _ in 0..2 {
            charges.extend([1, 1, 6, 2, 1, 3]);
        }
        let authentication: u64 = charges.iter().sum();
        assert_eq!(authentication, if reciprocal { 74 } else { 53 });
        for remaining in 0..authentication {
            let (mut heap, mut account, tile, _) = prefix(1, reciprocal)?;
            let function = if reciprocal {
                Elementwise::Reciprocal
            } else {
                Elementwise::SquareRoot
            };
            let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
            leave(&mut account, remaining)?;
            let before = account.usage().work;
            let pointer = std::rc::Rc::as_ptr(&request.input.record);
            let injection = crate::loan::fault::Injection::after(0);
            crate::loan::fault::reset_reads();
            match admit(&mut heap, request, function, &mut account) {
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    request,
                ) => {
                    assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
                }
                other => return Err(format!("{remaining}: {other:?}").into()),
            }
            let mut paid = 0;
            for charge in &charges {
                if paid + charge > remaining {
                    break;
                }
                paid += charge;
            }
            assert_eq!(account.usage().work - before, paid);
            assert_eq!(
                [
                    crate::loan::fault::Injection::attempts(),
                    crate::loan::fault::reads()
                ],
                [0; 2]
            );
            drop(injection);
        }
    }
    Ok(())
}

/// Complete traversal is 1+2(1+b)+2q+a*(17k+2), including empty demand and capacity.
#[test]
fn chain_evaluation_work_neighbours_preserve_ready() -> TestResult {
    for reciprocal in [false, true] {
        for (cells, selection, required) in [(3, 2, [64, 62]), (3, 0, [7, 5]), (0, 0, [5, 3])] {
            for short in [false, true] {
                evaluation_case(
                    reciprocal,
                    cells,
                    selection,
                    required[usize::from(reciprocal)],
                    short,
                )?;
            }
        }
    }
    Ok(())
}

/// First late-shell failure spends P; a second attempt independently needs P=207/247 at three cells.
#[test]
fn chain_repeated_publication_work_neighbours_never_reevaluate() -> TestResult {
    for reciprocal in [false, true] {
        for short in [false, true] {
            publication_case(reciprocal, short)?;
        }
    }
    Ok(())
}

/// A refused complete prepayment preserves every ready field and consumes only its fixed prefix.
fn evaluation_case(
    reciprocal: bool,
    cells: usize,
    selection: usize,
    required: u64,
    short: bool,
) -> TestResult {
    let (mut heap, mut account, tile, _) = prefix(cells, reciprocal)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(
        &mut heap,
        tile.into_pin(),
        function,
        selection,
        &mut account,
    )?;
    let owner = ready(admit(&mut heap, request, function, &mut account))?;
    let pointer = std::ptr::from_ref(owner.core.as_ref());
    let filled = owner.core.filled.clone();
    let outcomes = format!("{:?}", owner.core.evidence.outcomes);
    leave(&mut account, required - u64::from(short))?;
    let before = account.usage().work;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => {
            assert!(!short);
            owner
        }
        ApplicationEvaluation::ApplicationNotStarted(
            ApplicationError::Loan(LoanError::Budget(_)),
            owner,
        ) => {
            assert!(short);
            assert_eq!(owner.core.filled, filled);
            assert_eq!(format!("{:?}", owner.core.evidence.outcomes), outcomes);
            assert_eq!(
                [crate::loan::fault::reads(), crate::loan::fault::scalars()],
                [0; 2]
            );
            owner
        }
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
    assert_eq!(
        account.usage().work - before,
        if short {
            if reciprocal { 2 } else { 3 }
        } else {
            required
        }
    );
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    Ok(())
}

/// A repeated shell attempt pays the same publication tariff without another scalar invocation.
fn publication_case(reciprocal: bool, short: bool) -> TestResult {
    let (mut heap, mut account, tile, _) = prefix(3, reciprocal)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
    let owner = ready(admit(&mut heap, request, function, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let required = if reciprocal { 247 } else { 207 };
    leave(&mut account, 2 * required - u64::from(short))?;
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    let before = account.usage().work;
    let owner = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::AllocationRefused),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(account.usage().work - before, required);
    drop(injection);
    match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(_, _) => assert!(!short),
        ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::Budget(_)),
            owner,
        ) => {
            assert!(short);
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        other => return Err(format!("{other:?}").into()),
    }
    assert_eq!(crate::loan::fault::scalars(), 0);
    Ok(())
}

/// The separate account adds two debit visits to fixed publication 178/218.
/// Adding 3L+7q+1+C+3C gives 181/221 at C=0 and 195/235 at C=1, with k=1.
#[test]
fn chain_separate_account_publication_work_neighbours() -> TestResult {
    for (cells, required) in [(0, [181, 221]), (1, [195, 235])] {
        for reciprocal in [false, true] {
            for short in [false, true] {
                separate_publication(cells, reciprocal, required[usize::from(reciprocal)], short)?;
            }
        }
    }
    Ok(())
}

/// Acquire one new pin without reclaiming expired rows, then spend real Work to leave N or N-1.
/// The design's payload validation reads 3*k*z=3*C output values without invoking a kernel.
/// Both neighbours reach validation before the final nine prepared-record visits.
fn separate_publication(cells: usize, reciprocal: bool, required: u64, short: bool) -> TestResult {
    let (mut heap, _setup, tile, _) = prefix(cells, reciprocal)?;
    let ticket = tile.pin().ticket()?;
    drop(tile);
    let mut account =
        crate::ExecutionAccount::new(nrz_bindings::Budget::new(nrz_bindings::Limits::default())?)?;
    let input = heap.acquire_pin::<f64>(ticket, &mut account)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, input, function, 2, &mut account)?;
    let owner = ready(admit(&mut heap, request, function, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let pointer = std::ptr::from_ref(owner.core.as_ref());
    leave(&mut account, required - u64::from(short))?;
    let before = account.usage().work;
    assert!(before > 0);
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
    match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(_, _) => {
            assert!(!short);
            assert_eq!(account.usage().work - before, required);
        }
        ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::Budget(_)),
            owner,
        ) => {
            assert!(short);
            assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
        other => return Err(format!("{other:?}").into()),
    }
    assert_eq!(
        [crate::loan::fault::reads(), crate::loan::fault::scalars()],
        [3 * cells, 0]
    );
    drop(injection);
    Ok(())
}
