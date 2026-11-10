// SPDX-License-Identifier: AGPL-3.0-only
//! Refusals preserve the consumed graph, original cumulative account and exact retaining use.

use super::{TestResult, fixture};
use crate::{
    ExecutionAccount, Heap, HeapLimits, LoanError, PreparedExpm1Error, RetainedExpm1Error,
};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Location,
        enclosure::prepared_expm1::{Counts, Policy},
    },
};
use nrz_scalar::ArithmeticContext;

/// Domain, target and environment refusals return the producing graph with phase-accurate counts.
#[test]
fn retained_law_preparation_refusals_return_graph() -> TestResult {
    for mode in 0..4 {
        let (cas, mut request) = fixture::request()?;
        let root = request.original.target.clone();
        let before = cas.counts();
        let mut account = fixture::account()?;
        account
            .ledger
            .work(7, Location::new("retained refusal prefix"))?;
        let mut counts = Counts::default();
        let mut policy = Policy::default();
        let domain = if mode == 0 { [-2.0, 1.0] } else { [-2.0, 0.0] };
        if mode == 1 {
            request.environment = 2;
        }
        if mode == 2 {
            request.bits = 60;
            policy.attempts = 1;
        }
        if mode == 3 {
            counts.preparations = u64::MAX;
        }
        let injection = crate::loan::fault::Injection::after(usize::MAX);
        let mut heap = Heap::new(HeapLimits::default())?;
        let (error, returned) = heap
            .prepare_retained_expm1(
                cas,
                &request,
                domain,
                policy,
                &ArithmeticContext::new()?,
                &mut counts,
                &mut account,
            )
            .expect_err("preparation refuses");
        returned.expression(&root)?;
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(heap.loans.debits.len(), 0);
        match mode {
            0 => {
                assert!(matches!(
                    error,
                    RetainedExpm1Error::Preparation(PreparedExpm1Error::Domain)
                ));
                assert!(returned.counts().payload_bytes > before.payload_bytes);
                assert_eq!(counts.publications, 1);
            }
            1 | 3 => assert!(matches!(
                error,
                RetainedExpm1Error::Preparation(PreparedExpm1Error::Exact(_))
            )),
            2 => assert!(matches!(
                error,
                RetainedExpm1Error::Preparation(PreparedExpm1Error::TargetNotCertifiable)
            )),
            _ => return Err("fixture mode".into()),
        }
        assert!(account.usage().work >= 7);
        drop(injection);
    }
    Ok(())
}
/// A foreign heap/account cannot mint, inspect or consume an existing law edge.
#[test]
fn retained_law_foreign_authentication_preserves_use() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut other_heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let mut other_account = fixture::account()?;
    let mut law = fixture::prepare(&mut heap, &mut account)?;
    let pointer = std::rc::Rc::as_ptr(&law.record);
    assert!(matches!(
        heap.retain_retained_expm1(&law, &mut other_account),
        Err(LoanError::ForeignAccount)
    ));
    assert!(matches!(
        other_heap.retain_retained_expm1(&law, &mut account),
        Err(LoanError::ForeignOwner)
    ));
    assert!(matches!(
        heap.retained_expm1_inventory(&law, &mut other_account),
        Err(LoanError::ForeignAccount)
    ));
    let binding = super::super::Binding::of(&law)?;
    for _ in 0..3 {
        let before = other_account.usage().work;
        let (error, returned) = heap
            .release_retained_expm1(law, &mut other_account)
            .expect_err("foreign account release");
        assert!(matches!(error, LoanError::ForeignAccount));
        assert_eq!(other_account.usage().work, before + 1);
        assert_eq!(std::rc::Rc::as_ptr(&returned.record), pointer);
        assert_eq!(super::super::Binding::of(&returned)?, binding);
        law = returned;
        let before = account.usage().work;
        let (error, returned) = other_heap
            .release_retained_expm1(law, &mut account)
            .expect_err("foreign heap release");
        assert!(matches!(error, LoanError::ForeignOwner));
        assert_eq!(account.usage().work, before + 1);
        assert_eq!(std::rc::Rc::as_ptr(&returned.record), pointer);
        assert_eq!(super::super::Binding::of(&returned)?, binding);
        law = returned;
    }
    let before = account.usage().work;
    heap.release_retained_expm1(law, &mut account)
        .map_err(|(error, _)| error)?;
    assert_eq!(account.usage().work, before);
    Ok(())
}
/// Exhausted foreign probes return the same use; its legitimate prepaid release still succeeds.
#[test]
fn retained_law_rejected_release_work_exhaustion() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut other_heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let mut other_account = fixture::account()?;
    let mut law = fixture::prepare(&mut heap, &mut account)?;
    let pointer = std::rc::Rc::as_ptr(&law.record);
    let binding = super::super::Binding::of(&law)?;
    fixture::leave_work(&mut account, 0)?;
    fixture::leave_work(&mut other_account, 0)?;
    for _ in 0..3 {
        let before = other_account.usage().work;
        let (error, returned) = heap
            .release_retained_expm1(law, &mut other_account)
            .expect_err("exhausted foreign account release");
        fixture::work_refusal(error, other_account.limits().work, before + 1)?;
        assert_eq!(other_account.usage().work, before);
        assert_eq!(std::rc::Rc::as_ptr(&returned.record), pointer);
        assert_eq!(super::super::Binding::of(&returned)?, binding);
        law = returned;
        let before = account.usage().work;
        let (error, returned) = other_heap
            .release_retained_expm1(law, &mut account)
            .expect_err("exhausted foreign heap release");
        fixture::work_refusal(error, account.limits().work, before + 1)?;
        assert_eq!(account.usage().work, before);
        assert_eq!(std::rc::Rc::as_ptr(&returned.record), pointer);
        assert_eq!(super::super::Binding::of(&returned)?, binding);
        law = returned;
    }
    let before = account.usage().work;
    heap.release_retained_expm1(law, &mut account)
        .map_err(|(error, _)| error)?;
    assert_eq!(account.usage().work, before);
    Ok(())
}
/// Revision exhaustion refuses before allocation and leaves expired residence conservative.
#[test]
fn retained_law_revision_exhaustion() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let law = fixture::prepare(&mut heap, &mut account)?;
    heap.loans.revision = u64::MAX;
    let injection = crate::loan::fault::Injection::after(usize::MAX);
    assert!(matches!(
        heap.retain_retained_expm1(&law, &mut account),
        Err(LoanError::RevisionCapacity)
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(law);
    assert!(matches!(
        heap.residence(&mut account, 1000),
        Err(LoanError::RevisionCapacity)
    ));
    assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
    drop(injection);
    Ok(())
}
/// A spent original account cannot reset its preparation allowance through ownership transfer.
#[test]
fn retained_law_original_work_prefix() -> TestResult {
    let (cas, request) = fixture::request()?;
    let mut budget = Budget::new(nrz_bindings::Limits {
        work: 7,
        ..fixture::limits()
    })?;
    budget.work(7, Location::new("spent original account"))?;
    let mut account = ExecutionAccount::new(budget)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let (error, returned) = heap
        .prepare_retained_expm1(
            cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &ArithmeticContext::new()?,
            &mut Counts::default(),
            &mut account,
        )
        .expect_err("spent preparation ledger");
    assert!(matches!(
        error,
        RetainedExpm1Error::Preparation(PreparedExpm1Error::Exact(_))
    ));
    assert_eq!(account.usage().work, 7);
    returned.expression(&request.original.target)?;
    Ok(())
}
