// SPDX-License-Identifier: AGPL-3.0-only
//! Independent native owner counts pin the complete pending reservation and its refused prefix.

use super::{TestResult, demand, request, setup};
use crate::{LcOmegaOutput, LoanError};
use nrz_bindings::{
    Limits,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_scalar::Scalar;

/// Seven role checks plus six demand/edge visits, three declaration visits and 156 native visits.
/// ´test:unit:loan-lc-output-work-boundaries´
#[test]
fn loan_lc_output_work_boundaries() -> TestResult {
    work::<f32>()?;
    work::<f64>()
}
fn work<P: Scalar + 'static>() -> TestResult {
    for remaining in [171, 172] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let schema = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let demand = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let spend = s.account.limits().work - s.account.usage().work - remaining;
        s.account.ledger.work(spend, crate::loan::AT)?;
        let before = s.account.usage().work;
        crate::loan::fault::reset_reads();
        let injection = (remaining == 171).then(|| crate::loan::fault::Injection::after(0));
        let result = schema.reserve(&mut s.heap, &s.axes, demand, &mut s.account);
        if remaining == 172 {
            let mut pending = result?;
            assert_eq!(s.account.usage().work - before, 172);
            let ticket = pending.ticket()?;
            assert_eq!(pending.cancel(&mut s.heap, &mut s.account)?.ticket, ticket);
            assert_eq!(s.account.usage().work - before, 172);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
            assert_eq!(s.account.usage().work - before, 23);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        }
        assert_eq!(crate::loan::fault::reads(), 0);
        drop(injection);
    }
    Ok(())
}

/// One slot has one generation and one optional occupation, independently of its content.
#[repr(C)]
struct Slot<T> {
    generation: u64,
    value: Option<T>,
}
/// One shared allocation has two counters before its aligned value.
#[repr(C)]
struct Shared<T> {
    strong: usize,
    weak: usize,
    value: T,
}
/// Fixed 4096-cell, three-channel reservation; the existing demand owns one mask/layout slot.
fn bytes<P: Scalar>() -> [usize; 2] {
    use crate::heap::{descriptors::DescriptorRecord, identity::Key};
    use crate::loan::registry::{Debit, Liveness, PinIndex, PinRecord};
    use crate::mask::storage::{LayoutRecord, MaskRecord, Native};
    let mask_slots =
        size_of::<Slot<Native<LayoutRecord>>>() + size_of::<Slot<Native<MaskRecord>>>();
    let retained = 3 * (4096 * size_of::<P>() + 2 * 64 * 8)
        + crate::loan::abi::host_header()
        + crate::loan::abi::bundle_header()
        + size_of::<Shared<LayoutRecord>>()
        + size_of::<Shared<Liveness>>()
        + size_of::<Shared<PinRecord>>()
        + 3 * size_of::<Slot<DescriptorRecord>>()
        + mask_slots
        + 3 * crate::loan::abi::group_row()
        + 14 * size_of::<Option<Debit>>()
        + size_of::<Option<PinIndex>>();
    let scratch = 14 * size_of::<Debit>()
        + 5 * size_of::<Key>()
        + 3 * size_of::<DescriptorRecord>()
        + size_of::<Native<LayoutRecord>>()
        + size_of::<Native<MaskRecord>>()
        + 2 * mask_slots;
    [retained, retained + scratch]
}
/// Retained and staged limits reject before the first native allocation on the original ledger.
/// ´test:unit:loan-lc-output-byte-boundaries´
#[test]
fn loan_lc_output_byte_boundaries() -> TestResult {
    byte_boundaries::<f32>()?;
    byte_boundaries::<f64>()
}
fn byte_boundaries<P: Scalar + 'static>() -> TestResult {
    for staged in [false, true] {
        let required = u64::try_from(bytes::<P>()[usize::from(staged)])?;
        for limit in [required - 1, required] {
            let mut limits = Limits::default();
            if staged {
                limits.staged_payload_bytes = limit;
            } else {
                limits.output_bytes = limit;
            }
            let mut s = setup::<P>(limits, [2048, 1, 2])?;
            let schema = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let demand = demand(&mut s.heap, &s.axes, &[0, 4095], &mut s.account)?;
            let before = s.account.usage().work;
            let injection = (limit < required).then(|| crate::loan::fault::Injection::after(0));
            let result = schema.reserve(&mut s.heap, &s.axes, demand, &mut s.account);
            if limit == required {
                let mut pending = result?;
                pending.cancel(&mut s.heap, &mut s.account)?;
            } else {
                let Err(LoanError::Heap(error)) = result else {
                    return Err("expected output byte refusal".into());
                };
                assert_eq!(
                    error,
                    crate::HeapError::ResourceBytes {
                        staged,
                        used: required,
                        limit
                    }
                );
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                assert_eq!(s.heap.loans.elements()?, [0, 0]);
            }
            assert!(s.account.usage().work > before);
            drop(injection);
            super::fixture::retire(&mut s.heap, &mut s.account, demand)?;
        }
    }
    Ok(())
}

/// Seven LC visits plus 3046 frozen-environment and 7915 graph-admission visits.
/// ´test:unit:loan-lc-output-declaration-work-boundaries´
#[test]
fn loan_lc_output_declaration_work_boundaries() -> TestResult {
    declaration_work::<f32>()?;
    declaration_work::<f64>()
}
fn declaration_work<P: Scalar>() -> TestResult {
    for remaining in [10_967, 10_968] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let spend = s.account.limits().work - s.account.usage().work - remaining;
        s.account.ledger.work(spend, crate::loan::AT)?;
        let before = s.account.usage().work;
        let injection = crate::loan::fault::Injection::after(0);
        let result = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        );
        if remaining == 10_968 {
            let schema = result?;
            assert_eq!(schema.symbols().value().shape(), [12, 1]);
            assert_eq!(s.account.usage().work - before, 10_968);
        } else {
            let Err(crate::LcShapeError::Declaration(nrz_bindings::AdmissionError::Algebra(fault))) =
                result
            else {
                return Err("expected declaration Work refusal".into());
            };
            let FaultKind::Exhausted {
                budget,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                return Err("expected exhausted Work".into());
            };
            assert_eq!(budget, BudgetKind::Work);
            assert_eq!(limit, s.account.limits().work);
            assert_eq!(needed, before + 10_968);
            assert_eq!(fault.location.operation, "retained kind count");
            // Graph rollback preserves Work; the final two-Work count
            // refuses with one Work remaining.
            assert_eq!(s.account.usage().work - before, 10_966);
        }
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(s.heap.loans.elements()?, [0, 0]);
        drop(injection);
    }
    Ok(())
}

/// Cell inspection and refused publication prepay their full one/five visits.
/// ´test:unit:loan-lc-output-pending-work-boundaries´
#[test]
fn loan_lc_output_pending_work_boundaries() -> TestResult {
    pending_work::<f32>()?;
    pending_work::<f64>()
}
fn pending_work<P: Scalar + 'static>() -> TestResult {
    for (publication, required) in [(false, 1), (true, 5)] {
        for remaining in [required - 1, required] {
            let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
            let schema = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let selected = demand(&mut s.heap, &s.axes, &[3], &mut s.account)?;
            let mut pending = schema.reserve(&mut s.heap, &s.axes, selected, &mut s.account)?;
            let ticket = pending.ticket()?;
            let spend = s.account.limits().work - s.account.usage().work - remaining;
            s.account.ledger.work(spend, crate::loan::AT)?;
            let before = s.account.usage().work;
            let injection = crate::loan::fault::Injection::after(0);
            if publication {
                let result = pending.publish(&s.heap, &mut s.account);
                if remaining == required {
                    assert!(matches!(
                        result,
                        Err(crate::LcOutputPublicationError::OriginUnavailable)
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(crate::LcOutputPublicationError::Loan(LoanError::Budget(_)))
                    ));
                }
            } else {
                let result = pending.cell_state(3, &mut s.account);
                if remaining == required {
                    assert_eq!(result?, crate::LcOutputCellState::Pending);
                } else {
                    assert!(matches!(result, Err(LoanError::Budget(_))));
                }
            }
            let expected = if remaining == required { required } else { 0 };
            assert_eq!(s.account.usage().work - before, expected);
            assert_eq!(pending.ticket()?, ticket);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(pending.cancel(&mut s.heap, &mut s.account)?.ticket, ticket);
            assert_eq!(s.account.usage().work - before, expected);
            drop(injection);
        }
    }
    Ok(())
}
