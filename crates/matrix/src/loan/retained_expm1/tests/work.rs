// SPDX-License-Identifier: AGPL-3.0-only
//! Independent visit counts pin isolated shells, use admission, inventory and lifecycle work.

use super::super::{Attachment, Binding, admission};
use super::{TestResult, fixture};
use crate::{Heap, HeapLimits, LoanError, Residence, ScanOutcome};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{Exhaustion, FaultKind, Location, limits::BudgetKind},
};

/// Existing Work tariff location for explicit caller consumption.
const AT: Location = Location::new("retained law Work boundary fixture");

/// Isolated shell admission pays one authentication, three allocations, four record steps and two edges.
/// This ten-step control excludes preparation, lowering and retained-graph registration Work;
/// it is not the complete-constructor Work boundary.
#[test]
fn retained_law_isolated_shell_work_boundary() -> TestResult {
    for short in [0, 1] {
        let mut budget = Budget::new(Limits {
            work: 17 - short,
            ..fixture::limits()
        })?;
        budget.work(7, AT)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let injection = crate::loan::fault::Injection::after(usize::MAX);
        let result = admission::shells(&mut heap, 17, 0, &mut budget, fixture::seal());
        if short == 0 {
            drop(result?);
            assert_eq!(budget.usage().work, 17);
            assert_eq!(crate::loan::fault::Injection::attempts(), 3);
        } else {
            let LoanError::Budget(fault) = result.err().ok_or("managed Work refusal")? else {
                return Err("typed Work refusal".into());
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::Work,
                    limit: 16,
                    needed: 17
                }
            );
            assert_eq!(budget.usage().work, 8);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        }
        drop(injection);
    }
    Ok(())
}
/// First growth and reconciled-slot reuse pay every visit before installing another use.
#[test]
fn retained_law_additional_use_work_boundaries() -> TestResult {
    for spare in [false, true] {
        // Four authentication steps; one vacancy and two resource visits per debit slot.
        let rows = 2 + u64::from(spare);
        let authentication = 1 + 1 + 2;
        let visits = rows + 2 * rows;
        // One use allocation, an optional table replacement, two record and two edge steps.
        let cleanup = 1 + u64::from(!spare) + 2 + 2;
        let work = authentication + visits + cleanup;
        for short in [0, 1] {
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut account = fixture::account()?;
            let limits = account.limits().clone();
            let law = fixture::prepare(&mut heap, &mut account)?;
            if spare {
                drop(heap.retain_retained_expm1(&law, &mut account)?);
                assert!(matches!(
                    heap.reconcile_detached(&mut account, 1000, None),
                    ScanOutcome::Complete
                ));
            }
            assert_eq!(u64::try_from(heap.loans.debits.len())?, rows);
            assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
            let before = (
                heap.resource_bytes()?,
                heap.loans.debits.len(),
                heap.loans.debits.capacity(),
                heap.loans.revision,
            );
            let binding = Binding::of(&law)?;
            let authority = std::ptr::from_ref(law.authority()?);
            let prior = fixture::leave_work(&mut account, work - short)?;
            let injection = crate::loan::fault::Injection::after(usize::MAX);
            let result = heap.retain_retained_expm1(&law, &mut account);
            if short == 0 {
                let extra = result?;
                assert_eq!(account.usage().work, prior + work);
                assert_eq!(
                    crate::loan::fault::Injection::attempts(),
                    1 + usize::from(!spare)
                );
                assert_eq!(heap.loans.debits.iter().flatten().count(), 3);
                assert_eq!(std::ptr::from_ref(extra.authority()?), authority);
                if spare {
                    assert_eq!(extra.record.token.slot, 2);
                    assert_eq!(heap.loans.debits.capacity(), before.2);
                }
                heap.release_retained_expm1(extra, &mut account)
                    .map_err(|(error, _)| error)?;
                assert_eq!(account.usage().work, prior + work);
            } else {
                fixture::work_refusal(
                    result.expect_err("additional-use Work boundary"),
                    limits.work,
                    prior + work,
                )?;
                assert_eq!(account.usage().work, prior + authentication + visits);
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
                assert_eq!(
                    (
                        heap.resource_bytes()?,
                        heap.loans.debits.len(),
                        heap.loans.debits.capacity(),
                        heap.loans.revision,
                    ),
                    before
                );
            }
            assert_eq!(Binding::of(&law)?, binding);
            assert_eq!(std::ptr::from_ref(law.authority()?), authority);
            law.evaluate(fixture::point(0.0), &nrz_scalar::ArithmeticContext::new()?)?;
            assert_eq!(account.limits(), &limits);
            let before = account.usage().work;
            heap.release_retained_expm1(law, &mut account)
                .map_err(|(error, _)| error)?;
            assert_eq!(account.usage().work, before);
            drop(injection);
        }
    }
    Ok(())
}
/// Inventory visits every debit slot, including a reconciled vacancy beside multiple live uses.
#[test]
fn retained_law_inventory_work_boundaries() -> TestResult {
    for several in [false, true] {
        let rows = 2 + 2 * u64::from(several);
        let work = (1 + 1 + 2) + rows;
        for short in [0, 1] {
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut account = fixture::account()?;
            let limits = account.limits().clone();
            let law = fixture::prepare(&mut heap, &mut account)?;
            let extra = if several {
                let extra = heap.retain_retained_expm1(&law, &mut account)?;
                drop(heap.retain_retained_expm1(&law, &mut account)?);
                assert!(matches!(
                    heap.reconcile_detached(&mut account, 1000, None),
                    ScanOutcome::Complete
                ));
                Some(extra)
            } else {
                None
            };
            assert_eq!(u64::try_from(heap.loans.debits.len())?, rows);
            let binding = Binding::of(&law)?;
            let authority = std::ptr::from_ref(law.authority()?);
            let before = (heap.resource_bytes()?, heap.loans.revision);
            let prior = fixture::leave_work(&mut account, work - short)?;
            let injection = crate::loan::fault::Injection::after(usize::MAX);
            let result = heap.retained_expm1_inventory(&law, &mut account);
            if short == 0 {
                let inventory = result?;
                assert_eq!(inventory.owner_identity, binding.identity.owner);
                assert_eq!(inventory.use_identity, binding.use_identity);
                assert_eq!(inventory.uses, 1 + usize::from(several));
            } else {
                fixture::work_refusal(
                    result.expect_err("inventory Work boundary"),
                    limits.work,
                    prior + work,
                )?;
            }
            assert_eq!(account.usage().work, prior + work - short);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!((heap.resource_bytes()?, heap.loans.revision), before);
            assert_eq!(Binding::of(&law)?, binding);
            assert_eq!(std::ptr::from_ref(law.authority()?), authority);
            law.evaluate(fixture::point(0.0), &nrz_scalar::ArithmeticContext::new()?)?;
            assert_eq!(account.limits(), &limits);
            drop(extra);
            let before = account.usage().work;
            heap.release_retained_expm1(law, &mut account)
                .map_err(|(error, _)| error)?;
            assert_eq!(account.usage().work, before);
            drop(injection);
        }
    }
    Ok(())
}
/// Attachment checks and terminal authentication are admitted before moving the existing edge.
#[test]
fn retained_law_attachment_work_boundary() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    for short in [0, 1] {
        let law = fixture::prepare(&mut heap, &mut account)?;
        let expected = Binding::of(&law)?;
        let mut budget = Budget::new(Limits {
            work: 13 - short,
            ..fixture::limits()
        })?;
        budget.work(7, AT)?;
        let result = Attachment::prepare(law, expected, &heap, &mut budget);
        if short == 0 {
            let attachment = result.map_err(|(error, _)| error)?;
            assert_eq!(budget.usage().work, 13);
            attachment.validate_current(&heap)?;
        } else {
            let (error, returned) = result.expect_err("attachment prepayment");
            assert_eq!(Binding::of(&returned)?, expected);
            assert!(matches!(error, LoanError::Budget(_)));
            assert_eq!(budget.usage().work, 7);
        }
    }
    Ok(())
}
/// Expired owner/use visits each pay one slot and one index removal, with exact stopped cursors.
#[test]
fn retained_law_scan_work_boundaries() -> TestResult {
    for available in 0..=4 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut original = fixture::account()?;
        let law = fixture::prepare(&mut heap, &mut original)?;
        drop(law);
        let mut budget = Budget::new(Limits {
            work: 7 + available,
            ..fixture::limits()
        })?;
        budget.work(7, AT)?;
        let mut scanner = crate::ExecutionAccount::new(budget)?;
        let result = heap.reconcile_detached(&mut scanner, 2, None);
        assert_eq!(scanner.usage().work, 7 + available);
        assert_eq!(
            heap.loans.debits.iter().flatten().count(),
            2 - usize::try_from(available / 2)?
        );
        if available == 4 {
            assert!(matches!(result, ScanOutcome::Complete));
        } else {
            let ScanOutcome::WorkExhausted(cursor) = result else {
                return Err("scan Work cursor".into());
            };
            assert!(matches!(
                heap.reconcile_detached(&mut original, 2, Some(cursor)),
                ScanOutcome::Complete
            ));
        }
        assert!(matches!(
            heap.residence(&mut original, 1000)?,
            Residence::Empty { .. }
        ));
    }
    Ok(())
}
/// A live law with no numerical elements remains occupied; residence prepays all debit visits.
#[test]
fn retained_law_residence_work_boundary() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut original = fixture::account()?;
    let law = fixture::prepare(&mut heap, &mut original)?;
    // Two reconciliation slots, then four resource visits and two occupied-predicate visits.
    for available in [7, 8] {
        let mut budget = Budget::new(Limits {
            work: 7 + available,
            ..fixture::limits()
        })?;
        budget.work(7, AT)?;
        let mut scanner = crate::ExecutionAccount::new(budget)?;
        let result = heap.residence(&mut scanner, 2)?;
        if available == 8 {
            assert!(matches!(
                result,
                Residence::Pending {
                    elements: Some([0, 0]),
                    bytes: Some(_)
                }
            ));
            assert_eq!(scanner.usage().work, 15);
        } else {
            assert!(matches!(
                result,
                Residence::Pending {
                    elements: None,
                    bytes: None
                }
            ));
            assert_eq!(scanner.usage().work, 9);
        }
    }
    drop(law);
    Ok(())
}
