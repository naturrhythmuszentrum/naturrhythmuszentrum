// SPDX-License-Identifier: AGPL-3.0-only
//! Literal wrapper visits and independently enumerated invocation ceilings use the original ledger.

use super::{
    BinaryElementwise, Limits, TestResult, admit, candidate, fixture, leave, published, ready,
};
use crate::loan::fault;
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    BinaryApplicationRequest, ExecutionLimits, LoanError,
};

/// Fixed `15*h_left+21*h_right+233+11*I` and loops `6*L+3*t+(h+7)*q+C`.
#[test]
fn binary_admission_literal_work_neighbours() -> TestResult {
    for (cells, components, roles, required) in [
        (3, 1, [1, 1], 313),
        (3, 1, [2, 1], 329),
        (3, 1, [1, 2], 335),
        (3, 1, [3, 3], 389),
        (0, 1, [1, 1], 269),
        (1, 1, [1, 1], 299),
        (65, 3, [1, 1], 1545),
    ] {
        for short in [false, true] {
            let mut setup = fixture(
                cells,
                components,
                roles,
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
            leave(&mut setup.account, required - u64::from(short))?;
            let before = setup.account.usage().work;
            let injection = fault::Injection::after(if short { 0 } else { usize::MAX });
            match admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            ) {
                BinaryApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert_eq!(setup.account.usage().work - before, required);
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                BinaryApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    request,
                ) => {
                    assert!(short);
                    assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
                    assert_eq!(fault::Injection::attempts(), 0);
                    let total_roles = roles[0] + roles[1];
                    let debits = roles[0] + 3 * roles[1] + 10;
                    assert_eq!(
                        setup.account.usage().work - before,
                        u64::try_from(7 * total_roles + 57 + 3 * debits + 3 * cells * components)?
                    );
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
/// `C=3,k=h_left=h_right=1` has 122 metadata visits and a complete 191-step prepayment.
#[test]
fn binary_every_admission_work_prefix_preserves_both_pins() -> TestResult {
    for remaining in (0..=122).chain(std::iter::once(312)) {
        let mut setup = fixture(
            3,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let pins = [
            std::rc::Rc::as_ptr(&setup.request.left.record),
            std::rc::Rc::as_ptr(&setup.request.right.record),
        ];
        let before = setup.heap.resource_bytes()?;
        leave(&mut setup.account, remaining)?;
        let injection = fault::Injection::after(0);
        fault::reset_reads();
        fault::reset_scalars();
        fault::reset_stores();
        let request = match admit(
            &mut setup.heap,
            setup.request,
            BinaryElementwise::Product,
            &mut setup.account,
        ) {
            BinaryApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Budget(_)),
                request,
            ) => request,
            other => return Err(format!("prefix {remaining}: {other:?}").into()),
        };
        assert_eq!(
            [
                std::rc::Rc::as_ptr(&request.left.record),
                std::rc::Rc::as_ptr(&request.right.record)
            ],
            pins
        );
        assert_eq!(
            [
                fault::reads(),
                fault::scalars(),
                fault::stores(),
                fault::Injection::attempts()
            ],
            [0; 4]
        );
        assert_eq!(setup.heap.resource_bytes()?, before);
        drop(injection);
    }
    Ok(())
}
/// Two source visits and 2q+a*(21k+(ell+3)k+2) are charged atomically before any traversal.
#[test]
fn binary_evaluation_literal_work_and_unchanged_ready_neighbours() -> TestResult {
    for (cells, roles, selection, required) in [
        (3, [1, 1], 2, 88),
        (3, [2, 1], 2, 91),
        (3, [1, 3], 2, 91),
        (3, [3, 3], 2, 94),
        (3, [3, 3], 0, 4),
        (0, [1, 1], 0, 2),
    ] {
        for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
            for short in [false, true] {
                let mut setup = fixture(
                    cells,
                    1,
                    roles,
                    selection,
                    false,
                    function,
                    Limits::default(),
                )?;
                let owner = ready(admit(
                    &mut setup.heap,
                    setup.request,
                    function,
                    &mut setup.account,
                ))?;
                let pointer = std::ptr::from_ref(owner.core.as_ref());
                let words = owner.core.filled.clone();
                let outcomes = format!("{:?}", owner.core.evidence.outcomes);
                let right =
                    std::ptr::from_ref(owner.core.evidence.endpoint_tokens.binary().ok_or("side")?);
                leave(&mut setup.account, required - u64::from(short))?;
                let before = setup.account.usage().work;
                fault::reset_reads();
                fault::reset_scalars();
                fault::reset_stores();
                let owner = match owner.evaluate(&mut setup.account) {
                    ApplicationEvaluation::ApplicationCandidate(owner) => {
                        assert!(!short);
                        assert_eq!(setup.account.usage().work - before, required);
                        owner
                    }
                    ApplicationEvaluation::ApplicationNotStarted(
                        ApplicationError::Loan(LoanError::Budget(_)),
                        owner,
                    ) => {
                        assert!(short);
                        assert!(!owner.core.candidate);
                        assert_eq!(owner.core.filled, words);
                        assert_eq!(format!("{:?}", owner.core.evidence.outcomes), outcomes);
                        assert_eq!([fault::reads(), fault::scalars(), fault::stores()], [0; 3]);
                        owner
                    }
                    other => return Err(format!("{other:?}").into()),
                };
                assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
                assert_eq!(
                    std::ptr::from_ref(owner.core.evidence.endpoint_tokens.binary().ok_or("side")?),
                    right
                );
                assert!(matches!(
                    setup.heap.cancel_application(owner, &mut setup.account),
                    ApplicationCancellation::ApplicationCancelled(_)
                ));
            }
        }
    }
    Ok(())
}
/// Publication fixed 5*h+129 and loops 3L+7q+1+C+3kz; retries cannot rerun the kernel.
#[test]
fn binary_publication_literal_work_and_repair_neighbours() -> TestResult {
    for (cells, roles, required) in [
        (3, [1, 1], 168),
        (3, [2, 1], 173),
        (3, [3, 3], 188),
        (0, [1, 1], 140),
    ] {
        for short in [false, true] {
            let mut setup = fixture(
                cells,
                1,
                roles,
                2,
                false,
                BinaryElementwise::Quotient,
                Limits::default(),
            )?;
            let owner = candidate(
                ready(admit(
                    &mut setup.heap,
                    setup.request,
                    BinaryElementwise::Quotient,
                    &mut setup.account,
                ))?,
                &mut setup.account,
            )?;
            let injection = fault::Injection::after(0);
            let before = setup.account.usage().work;
            fault::reset_scalars();
            let owner = match setup.heap.publish_application(owner, &mut setup.account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(setup.account.usage().work - before, required);
            drop(injection);
            leave(&mut setup.account, required - u64::from(short))?;
            let before = setup.account.usage().work;
            match setup.heap.publish_application(owner, &mut setup.account) {
                ApplicationPublication::ApplicationPublished(_, _) => {
                    assert!(!short);
                    assert_eq!(setup.account.usage().work - before, required);
                }
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => {
                    assert!(short);
                    assert_eq!(setup.account.usage().work - before, required - 9);
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(fault::scalars(), 0);
        }
    }
    Ok(())
}
/// Both slots and independently limiting work/scratch bind even for zero capacity.
#[test]
fn binary_application_invocation_and_payload_limits() -> TestResult {
    for (cells, components, required) in [(0, 3, 63), (1, 3, 63), (65, 1, 195)] {
        for ceiling in 0..3 {
            for short in [false, true] {
                let mut setup = fixture(
                    cells,
                    components,
                    [1; 2],
                    2,
                    false,
                    BinaryElementwise::Product,
                    Limits::default(),
                )?;
                setup.request.limits = ExecutionLimits::new(
                    2 - usize::from(ceiling == 0 && short),
                    required - usize::from(ceiling == 1 && short),
                    required - usize::from(ceiling == 2 && short),
                );
                let injection = fault::Injection::after(if short { 0 } else { usize::MAX });
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
                        ApplicationError::Operation(crate::OperationError::ResourceLimit),
                        _,
                    ) => {
                        assert!(short);
                        assert_eq!(fault::Injection::attempts(), 0);
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
/// One additional correspondence visit gives four historical and 26 current reader visits.
#[test]
fn binary_current_historical_and_selected_reader_limits() -> TestResult {
    for historical in [false, true] {
        for short in [false, true] {
            let mut setup = fixture(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let owner = candidate(
                ready(admit(
                    &mut setup.heap,
                    setup.request,
                    BinaryElementwise::Product,
                    &mut setup.account,
                ))?,
                &mut setup.account,
            )?;
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let required = if historical { 4 } else { 26 };
            leave(&mut setup.account, required - u64::from(short))?;
            let before = setup.account.usage().work;
            fault::reset_reads();
            let result = if historical {
                tile.pin().historical_reader(&mut setup.account)
            } else {
                tile.reader(&setup.heap, &mut setup.account)
            };
            assert_eq!(result.is_ok(), !short);
            if !short {
                assert_eq!(setup.account.usage().work - before, required);
            }
            assert_eq!(fault::reads(), 0);
        }
    }
    Ok(())
}

/// Three pin visits precede ten owner/token visits, including the side token, before authentication.
#[test]
fn binary_current_reader_side_token_authentication_prefix() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for available in [12, 13, 26] {
            let mut setup = fixture(1, 1, [1; 2], 2, false, function, Limits::default())?;
            let owner = candidate(
                ready(admit(
                    &mut setup.heap,
                    setup.request,
                    function,
                    &mut setup.account,
                ))?,
                &mut setup.account,
            )?;
            let side_slot = owner
                .core
                .evidence
                .endpoint_tokens
                .binary()
                .ok_or("side")?
                .token
                .slot;
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let debit = setup.heap.loans.debits[side_slot]
                .take()
                .ok_or("side debit")?;
            leave(&mut setup.account, available)?;
            let before = setup.account.usage().work;
            fault::reset_reads();
            fault::reset_scalars();
            let result = tile.reader(&setup.heap, &mut setup.account);
            if available == 12 {
                assert!(matches!(result, Err(LoanError::Budget(_))), "{result:?}");
                assert_eq!(setup.account.usage().work - before, 3);
            } else {
                assert!(matches!(result, Err(LoanError::WrongLoan)), "{result:?}");
                assert_eq!(setup.account.usage().work - before, 13);
            }
            assert_eq!([fault::reads(), fault::scalars()], [0; 2]);
            setup.heap.loans.debits[side_slot] = Some(debit);
        }
    }
    Ok(())
}
